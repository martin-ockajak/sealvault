// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import SwiftUI

struct AppTabNavigation: View {
    @EnvironmentObject private var model: GlobalModel

    var body: some View {
        AppTabNavigationInner(callbackModel: model.callbackModel)
    }
}

struct AppTabNavigationInner: View {
    enum Tab {
        case profiles
        case browserOne
        case browserTwo
    }

    @EnvironmentObject private var model: GlobalModel
    @EnvironmentObject private var bannerModel: BannerModel

    @ObservedObject var callbackModel: CallbackModel
    @State var selection: Tab = .profiles
    @State var activeProfileId: String?
    @StateObject var browserModelOne = BrowserModel()
    @StateObject var browserModelTwo = BrowserModel()
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        TabView(selection: $selection) {
            Group {
                NavigationView {
                    BrowserView(browserModel: browserModelOne)
                }
                .tabItem {
                    Image(systemName: "network")
                        .accessibility(label: Text("Left Browser"))
                }
                .tag(Tab.browserOne)

                NavigationView {
                    ProfileListView()
                }
                .tabItem {
                    let label = Text("Profiles (\(model.activeProfile?.displayName ?? "No profile") is active)")
                    if let profile = model.activeProfile {
                        TabIcon(icon: profile.picture)
                            .accessibility(label: label)
                    } else {
                        Image(systemName: "person")
                            .accessibility(label: label)
                    }
                }
                .tag(Tab.profiles)

                NavigationView {
                    BrowserView(browserModel: browserModelTwo)
                }
                .tabItem {
                    Image(systemName: "network")
                        .accessibility(label: Text("Right Browser"))
                }
                .tag(Tab.browserTwo)
            }
        }
        .navigationViewStyle(StackNavigationViewStyle())
        .onAppear {
            let appearance = UITabBarAppearance()

            appearance.configureWithTransparentBackground()
            appearance.backgroundColor = UIColor(model.tabBarColor(colorScheme))

            UITabBar.appearance().standardAppearance = appearance
        }
        .onChange(of: callbackModel.tokenTransferSent) { val in
            guard let res = val else {
                return
            }
            if let errorMessage = res.errorMessage {
                let title = "Failed to transfer \(res.amount) \(res.tokenSymbol) to \(res.displayTo())"
                let detail = "Error on \(res.chainDisplayName): \(errorMessage)"
                bannerModel.bannerData = BannerData(title: title, detail: detail, type: .error)
            } else {
                let title = "Sent \(res.amount) \(res.tokenSymbol) to \(res.displayTo())"
                let details = "On \(res.chainDisplayName)"
                bannerModel.bannerData = BannerData(title: title, detail: details, type: .success)
            }
        }
        .onChange(of: callbackModel.tokenTransferResult) { val in
            guard let res = val else {
                return
            }
            if let errorMessage = res.errorMessage {
                let title = "Failed to transfer \(res.amount) \(res.tokenSymbol) to \(res.displayTo())"
                let detail = "Error on \(res.chainDisplayName): \(errorMessage)"
                bannerModel.bannerData = BannerData(title: title, detail: detail, type: .error)
            } else {
                let title = "Confirmed \(res.amount) \(res.tokenSymbol) to \(res.displayTo())"
                let details = "On \(res.chainDisplayName)"
                bannerModel.bannerData = BannerData(title: title, detail: details, type: .success)
            }
        }
        .onChange(of: callbackModel.dappAllotmentResult) { val in
            guard let res = val else {
                return
            }
            if let errorMessage = res.errorMessage {
                let title = "Failed to transfer \(res.amount) \(res.tokenSymbol) to \(res.dappIdentifier) address"
                let detail = "Error on \(res.chainDisplayName): \(errorMessage)"
                bannerModel.bannerData = BannerData(title: title, detail: detail, type: .error)
            } else {
                let title = "Confirmed \(res.amount) \(res.tokenSymbol) to \(res.dappIdentifier) address"
                let details = "On \(res.chainDisplayName)"
                bannerModel.bannerData = BannerData(title: title, detail: details, type: .success)
            }
        }
        .onChange(of: callbackModel.dappSignatureResult) { val in
            guard let res = val else {
                return
            }
            let title = "Approved signature for \(res.dappIdentifier)"
            let detail = "Automatic approval is safe because it has its own address."
            bannerModel.bannerData = BannerData(title: title, detail: detail, type: .success)
        }
        .onChange(of: callbackModel.dappTransactionApproved) { val in
            guard let res = val else {
                return
            }
            let title = "Approved transaction for \(res.dappIdentifier)"
            let detail = "Automatic approval is safe because it has its own address."
            bannerModel.bannerData = BannerData(title: title, detail: detail, type: .success)
        }
        .onChange(of: callbackModel.dappTransactionResult) { val in
            guard let res = val else {
                return
            }
            if let errorMessage = res.errorMessage {
                let title = "Transaction failed for \(res.dappIdentifier)"
                let detail = "Error on \(res.chainDisplayName): \(errorMessage)"
                bannerModel.bannerData = BannerData(title: title, detail: detail, type: .error)
            } else {
                // TODO add blockchain explorer url once it's tappable.
                let title = "Confirmed transaction for \(res.dappIdentifier)"
                let detail = ""
                bannerModel.bannerData = BannerData(title: title, detail: detail, type: .success)
            }
        }
        .banner(data: $bannerModel.bannerData)
        .edgesIgnoringSafeArea(.bottom)
        .onChange(of: selection) { _ in
            bannerModel.bannerData = nil
        }
        .onChange(of: activeProfileId) { [activeProfileId] _ in
            // Active profile id is nil when the app starts and the model is loading from the DB. We don't want to reset
            // then since the user didn't change the profile in this case.
            if activeProfileId == nil {
                return
            }
            // Disconnect dapps when active profile id changes. No action needed in Rust since in-page-provider
            // is stateless and works in the context of the currently active profile id.
            browserModelOne.clearHistory()
            browserModelTwo.clearHistory()
        }
        .onChange(of: model.activeProfileId) { newValue in
            self.activeProfileId = newValue
        }
        .onChange(of: model.leftBrowserURL) { newValue in
            if let url = newValue {
                browserModelOne.loadUrl(url)
                selection = .browserOne
            }
        }
        .onChange(of: model.rightBrowserURL) { newValue in
            if let url = newValue {
                browserModelTwo.loadUrl(url)
                selection = .browserTwo
            }
        }
    }
}

// We can only display a custom image in a tab item with SwiftUI only if the source is a UIImage and the modifiers
// must be set on the UIImage, as they have no effect when applied to the SwiftUI Image constructed from UIImage.
struct TabIcon: View {
    var icon: UIImage
    var size: CGSize = CGSize(width: 30, height: 30)

    // Based on https://stackoverflow.com/a/32303467
    var roundedIcon: UIImage {
        let rect = CGRect(origin: CGPoint(x: 0, y: 0), size: self.size)
        UIGraphicsBeginImageContextWithOptions(self.size, false, 1)
        defer {
            // End context after returning to avoid memory leak
            UIGraphicsEndImageContext()
        }

        UIBezierPath(
            roundedRect: rect,
            cornerRadius: self.size.height
        ).addClip()
        icon.draw(in: rect)
        return UIGraphicsGetImageFromCurrentImageContext()!
    }

    var body: some View {
        // Must set to original, otherwise it's just grey.
        Image(uiImage: roundedIcon.withRenderingMode(.alwaysOriginal))
    }
}

#if DEBUG
struct AppTabNavigation_Previews: PreviewProvider {
    static var previews: some View {
        let model = GlobalModel.buildForPreview()

        let explorerUrl = "https://etherscan.io/tx/0x24d3df3ce3eab3578e6486ebd6b071da3cc715780a1d0870b19ce8fde8e0f22a"

        let callbackTokenSent = CallbackModel()
        DispatchQueue.main.asyncAfter(deadline: .now() + .seconds(2)) {
            callbackTokenSent.tokenTransferSent = TokenTransferResult(
                amount: "0.1", tokenSymbol: "MATIC", chainDisplayName: "Polygon PoS",
                toDisplayName: "Default Profile Wallet", explorerUrl: nil, errorMessage: nil
            )
        }

        let callbackTokenTransferError = CallbackModel()
        DispatchQueue.main.asyncAfter(deadline: .now() + .seconds(2)) {
            callbackTokenTransferError.tokenTransferSent = TokenTransferResult(
                amount: "0.1", tokenSymbol: "MATIC", chainDisplayName: "Polygon PoS",
                toDisplayName: "Default Profile Wallet", explorerUrl: nil, errorMessage: "insufficient funds"
            )
        }

        let callbackTokenTransferConfirmed = CallbackModel()
        DispatchQueue.main.asyncAfter(deadline: .now() + .seconds(2)) {
            callbackTokenTransferConfirmed.tokenTransferResult = TokenTransferResult(
                amount: "0.1", tokenSymbol: "MATIC", chainDisplayName: "Polygon PoS",
                toDisplayName: "Default Profile Wallet", explorerUrl: explorerUrl, errorMessage: nil
            )
        }

        let callbackDappAllotmentSuccess = CallbackModel()
        DispatchQueue.main.asyncAfter(deadline: .now() + .seconds(2)) {
            callbackDappAllotmentSuccess.dappAllotmentResult = DappAllotmentTransferResult(
                dappIdentifier: "example.com", amount: "0.1", tokenSymbol: "MATIC",
                chainDisplayName: "Polygon PoS", errorMessage: nil
            )
        }

        let callbackDappAllotmentError = CallbackModel()
        DispatchQueue.main.asyncAfter(deadline: .now() + .seconds(2)) {
            callbackDappAllotmentError.dappAllotmentResult = DappAllotmentTransferResult(
                dappIdentifier: "example.com", amount: "0.1", tokenSymbol: "MATIC",
                chainDisplayName: "Polygon PoS", errorMessage: "insufficient funds"
            )
        }

        let callbackSignedMessage = CallbackModel()
        DispatchQueue.main.asyncAfter(deadline: .now() + .seconds(2)) {
            callbackSignedMessage.dappSignatureResult = DappSignatureResult(
                dappIdentifier: "example.com"
            )
        }

        let callbackSentTransaction = CallbackModel()
        DispatchQueue.main.asyncAfter(deadline: .now() + .seconds(2)) {
            callbackSentTransaction.dappTransactionApproved = DappTransactionApproved(
                dappIdentifier: "example.com", chainDisplayName: "Polygon PoS"
            )
        }

        let callbackDappTxResult = CallbackModel()
        DispatchQueue.main.asyncAfter(deadline: .now() + .seconds(2)) {
            callbackDappTxResult.dappTransactionResult = DappTransactionResult(
                dappIdentifier: "example.com", chainDisplayName: "Ethereum",
                explorerUrl: explorerUrl, errorMessage: nil
            )
        }

        let callbackDappTxError = CallbackModel()
        DispatchQueue.main.asyncAfter(deadline: .now() + .seconds(2)) {
            callbackDappTxError.dappTransactionResult = DappTransactionResult(
                dappIdentifier: "example.com", chainDisplayName: "Polygon PoS",
                explorerUrl: nil, errorMessage: "insufficient funds"
            )
        }

        return Group {
            AppTabNavigationInner(callbackModel: CallbackModel(), selection: .profiles)
                .environmentObject(model)

            AppTabNavigationInner(callbackModel: callbackTokenSent, selection: .profiles)
                .environmentObject(model)
            AppTabNavigationInner(callbackModel: callbackTokenTransferError, selection: .profiles)
                .environmentObject(model)
            AppTabNavigationInner(callbackModel: callbackTokenTransferConfirmed, selection: .profiles)
                .environmentObject(model)
            AppTabNavigationInner(callbackModel: callbackDappAllotmentSuccess, selection: .browserOne)
                .environmentObject(model)
            AppTabNavigationInner(callbackModel: callbackDappAllotmentError, selection: .profiles)
                .environmentObject(model)
            AppTabNavigationInner(callbackModel: callbackSignedMessage, selection: .profiles)
                .environmentObject(model)
            AppTabNavigationInner(callbackModel: callbackSentTransaction, selection: .profiles)
                .environmentObject(model)
            AppTabNavigationInner(callbackModel: callbackDappTxResult, selection: .profiles)
                .environmentObject(model)
            AppTabNavigationInner(callbackModel: callbackDappTxError, selection: .profiles)
                .environmentObject(model)
        }
    }
}
#endif
