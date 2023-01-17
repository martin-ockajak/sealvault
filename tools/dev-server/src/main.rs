// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::sync::Arc;

use axum::{
    body::{boxed, BoxBody},
    extract::State,
    http::{header, HeaderMap, Request, Response, StatusCode, Uri},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use dotenv::dotenv;
use ethers::core::utils::hex;
use hyper::Body;
use tower::ServiceExt;
use tower_http::{services::ServeDir, trace::TraceLayer};
use uniffi_sealvault_core::{
    async_runtime, AppCore, CoreArgs, CoreBackupStorageI, CoreInPageCallbackI,
    CoreUICallbackI, DappAllotmentTransferResult, DappApprovalParams,
    DappSignatureResult, DappTransactionApproved, DappTransactionResult,
    InPageRequestContextI, TokenTransferResult,
};

const DB_PATH: &str = ":memory:";
const STATIC_FOLDER: &str = "./static";
const ADDRESS: &str = "127.0.0.1:8080";

/// SealVault Dev Server
///
/// Serves the static directory at `http://localhost:8080/` and proxies requests to the backend
/// at http://localhost:8080/backend
///
fn main() {
    dotenv().ok();

    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let backend_args = CoreArgs {
        device_id: "dev-server".into(),
        device_name: "dev-server".into(),
        cache_dir: "./cache".into(),
        db_file_path: DB_PATH.into(),
    };
    let app_core = Arc::new(
        AppCore::new(
            backend_args,
            Box::new(CoreBackupStorageMock::new()),
            Box::new(CoreUICallBackMock::new()),
        )
        .expect("core initializes"),
    );

    async_runtime::block_on(run_server(app_core));
}

async fn run_server(app_core: Arc<AppCore>) {
    let app = Router::new()
        .route("/backend", post(backend))
        .route("/js/in-page-provider.js", get(in_page_provider))
        .fallback(static_handler)
        .layer(TraceLayer::new_for_http())
        .with_state(app_core);

    axum::Server::bind(&ADDRESS.parse().expect("valid address"))
        .serve(app.into_make_service())
        .await
        .expect("server starts");
}

// Based on https://benw.is/posts/serving-static-files-with-axum
async fn static_handler(
    uri: Uri,
    headers: HeaderMap,
) -> Result<Response<BoxBody>, (StatusCode, String)> {
    dbg!(&uri);
    let res = get_static_file(uri.clone()).await?;

    let content_type = get_header_value(res.headers(), "Content-Type");
    if content_type.to_lowercase().contains("html") {
        let bytes = hyper::body::to_bytes(res.into_body())
            .await
            .expect("can consume body");
        let mut body_str =
            String::from_utf8(bytes.to_vec()).expect("body bytes is valid utf-8");
        let user_agent = get_header_value(&headers, "User-Agent").to_lowercase();
        if !user_agent.contains("iphone") {
            body_str = body_str.replace("<!--desktop-only", "");
            body_str = body_str.replace("desktop-only-->", "");
        };
        let html_response = Response::builder()
            .status(200)
            .header("Content-Type", "text/html")
            .body(body_str)
            .expect("html response valid");
        Ok(html_response.map(boxed))
    } else {
        Ok(res)
    }
}

async fn get_static_file(uri: Uri) -> Result<Response<BoxBody>, (StatusCode, String)> {
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();

    match ServeDir::new(STATIC_FOLDER).oneshot(req).await {
        Ok(res) => Ok(res.map(boxed)),
        Err(err) => {
            log::error!("Error serving directory: {err}");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error".to_string(),
            ))
        }
    }
}

async fn in_page_provider(State(app_core): State<Arc<AppCore>>) -> impl IntoResponse {
    const SEALVAULT_RPC_PROVIDER: &str = "sealVaultRpcProvider";
    const SEALVAULT_REQUEST_HANDLER: &str = "sealVaultRequestHandler";

    let in_page_script = app_core.get_in_page_script(
        SEALVAULT_RPC_PROVIDER.into(),
        SEALVAULT_REQUEST_HANDLER.into(),
    );

    match in_page_script {
        Ok(contents) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/javascript")],
            contents,
        ),
        Err(err) => {
            log::error!("Error loading in page script: {err}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "application/javascript")],
                "".to_string(),
            )
        }
    }
}

async fn backend(
    State(app_core): State<Arc<AppCore>>,
    headers: HeaderMap,
    req_body: String,
) -> impl IntoResponse {
    let referer = get_header_value(&headers, "Referer");

    // TODO support respond and notify
    let in_page_request_context = Box::new(InPageRequestContextMock::new(&referer));
    let result = tokio::task::spawn_blocking(move || {
        app_core.in_page_request(in_page_request_context, req_body)
    })
    .await
    .expect("thread can be joined");

    match result {
        Ok(_) => StatusCode::OK,
        Err(err) => {
            log::error!("Error processing in page request: {err}");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

fn get_header_value(headers: &HeaderMap, name: &str) -> String {
    let default_value: header::HeaderValue = header::HeaderValue::from_str("").unwrap();
    let referer = headers
        .get(name)
        .unwrap_or(&default_value)
        .to_str()
        .expect("referrer is valid utf-8");
    referer.to_string()
}

#[derive(Debug, Default)]
pub struct CoreUICallBackMock {}

impl CoreUICallBackMock {
    pub fn new() -> Self {
        Self {}
    }
}

impl CoreUICallbackI for CoreUICallBackMock {
    fn sent_token_transfer(&self, result: TokenTransferResult) {
        log::info!("Sent token transfer: {:?}", result)
    }

    fn token_transfer_result(&self, result: TokenTransferResult) {
        log::info!("Token transfer result: {:?}", result)
    }

    fn dapp_allotment_transfer_result(&self, result: DappAllotmentTransferResult) {
        log::info!("Dapp allotment transfer result: {:?}", result)
    }

    fn signed_message_for_dapp(&self, result: DappSignatureResult) {
        log::info!("Dapp signature result: {:?}", result)
    }

    fn approved_dapp_transaction(&self, result: DappTransactionApproved) {
        log::info!("Sent transactions for dapp result: {:?}", result)
    }

    fn dapp_transaction_result(&self, result: DappTransactionResult) {
        log::info!("Dapp transaction result: {:?}", result)
    }
}

#[derive(Debug)]
pub struct InPageRequestContextMock {
    pub page_url: String,
    pub callbacks: Box<CoreInPageCallbackMock>,
}

impl InPageRequestContextMock {
    pub fn new(page_url: &str) -> Self {
        Self {
            page_url: page_url.into(),
            callbacks: Box::new(CoreInPageCallbackMock::new()),
        }
    }
}

impl InPageRequestContextI for InPageRequestContextMock {
    fn page_url(&self) -> String {
        self.page_url.clone()
    }

    fn callbacks(&self) -> Box<dyn CoreInPageCallbackI> {
        self.callbacks.clone()
    }
}

#[derive(Debug, Clone)]
pub struct CoreInPageCallbackMock {}

impl CoreInPageCallbackMock {
    // We don't want to create the mock by accident with `Default::default`.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {}
    }
}

impl CoreInPageCallbackI for CoreInPageCallbackMock {
    fn request_dapp_approval(&self, _: DappApprovalParams) {
        // Don't slow down tests noticeably, but simulate blocking.
        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    fn respond(&self, response_hex: String) {
        let response = hex::decode(response_hex).expect("valid hex");
        let response = String::from_utf8_lossy(&response);
        log::info!("CoreInPageCallbackMock.response: '{:?}'", response);
    }

    fn notify(&self, message_hex: String) {
        let event = hex::decode(message_hex).expect("valid hex");
        let event = String::from_utf8_lossy(&event);
        log::info!("CoreInPageCallbackMock.notify: '{:?}'", event);
    }
}

#[derive(Debug, Clone)]
pub struct CoreBackupStorageMock {}

impl CoreBackupStorageMock {
    // We don't want to create the mock by accident with `Default::default`.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {}
    }
}

impl CoreBackupStorageI for CoreBackupStorageMock {
    fn can_backup(&self) -> bool {
        false
    }

    fn is_uploaded(&self, _: String) -> bool {
        false
    }

    fn list_backup_file_names(&self) -> Vec<String> {
        Default::default()
    }

    fn copy_to_storage(&self, _: String, _: String) -> bool {
        false
    }

    fn copy_from_storage(&self, _: String, _: String) -> bool {
        false
    }

    fn delete_backup(&self, _: String) -> bool {
        false
    }
}
