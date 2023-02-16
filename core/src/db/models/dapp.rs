// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use diesel::{prelude::*, SqliteConnection};
use generic_array::{typenum::U1, GenericArray};
use url::Url;

use crate::{
    db::{
        deterministic_id::{DeriveDeterministicId, DeterministicId, EntityName},
        schema::{asymmetric_keys, dapps, profiles},
        url_value::UrlValue,
        DeferredTxConnection,
    },
    public_suffix_list::PublicSuffixList,
    utils::rfc3339_timestamp,
    Error,
};

#[derive(Clone, Debug, PartialEq, Eq, Queryable, Identifiable)]
#[diesel(primary_key(deterministic_id))]
pub struct Dapp {
    pub deterministic_id: DeterministicId,
    pub identifier: String,
    pub url: UrlValue,
    pub created_at: String,
    pub updated_at: Option<String>,
}

type AllColumns = (
    dapps::deterministic_id,
    dapps::identifier,
    dapps::url,
    dapps::created_at,
    dapps::updated_at,
);

const ALL_COLUMNS: AllColumns = (
    dapps::deterministic_id,
    dapps::identifier,
    dapps::url,
    dapps::created_at,
    dapps::updated_at,
);

impl Dapp {
    pub fn all_columns() -> AllColumns {
        ALL_COLUMNS
    }

    pub fn list_all(conn: &mut SqliteConnection) -> Result<Vec<Self>, Error> {
        Ok(dapps::table.load::<Self>(conn)?)
    }

    /// List all dapps that have been added to an profile.
    pub fn list_for_profile(
        conn: &mut SqliteConnection,
        profile_id: &DeterministicId,
    ) -> Result<Vec<Self>, Error> {
        use asymmetric_keys::dsl as ak;
        use dapps::dsl as d;

        let dapps: Vec<Self> = asymmetric_keys::table
            .inner_join(dapps::table.on(ak::dapp_id.eq(d::deterministic_id.nullable())))
            .filter(ak::profile_id.eq(profile_id))
            .select(Self::all_columns())
            .load(conn)?;

        Ok(dapps)
    }

    /// List dapp ids in descending order by last updated at.
    pub fn list_dapp_ids_desc(
        conn: &mut SqliteConnection,
        limit: u32,
    ) -> Result<Vec<DeterministicId>, Error> {
        use dapps::dsl as d;

        let dapp_ids: Vec<DeterministicId> = dapps::table
            .select(d::deterministic_id)
            .order((d::updated_at.desc(), d::created_at.desc()))
            .limit(limit as i64)
            .load(conn)?;

        Ok(dapp_ids)
    }

    /// Get the human-readable dapp identifier from an url.
    pub fn dapp_identifier(
        url: Url,
        public_suffix_list: &PublicSuffixList,
    ) -> Result<String, Error> {
        let dapp_entity = DappEntity::new(url, public_suffix_list)?;
        Ok(dapp_entity.identifier)
    }

    /// Get the human-readable dapp identifier for a dapp id.
    pub fn fetch_dapp_identifier(
        conn: &mut SqliteConnection,
        dapp_id: &DeterministicId,
    ) -> Result<String, Error> {
        use dapps::dsl as d;

        let identifier = dapps::table
            .filter(d::deterministic_id.eq(dapp_id))
            .select(d::identifier)
            .first(conn)?;

        Ok(identifier)
    }

    /// Create a dapp entity and return its deterministic id.
    /// The operation is idempotent.
    pub fn create_if_not_exists(
        tx_conn: &mut DeferredTxConnection,
        url: Url,
        public_suffix_list: &PublicSuffixList,
    ) -> Result<DeterministicId, Error> {
        let dapp_entity = DappEntity::new(url, public_suffix_list)?;
        let dapp_id = dapp_entity.create_if_not_exists(tx_conn.as_mut())?;
        Ok(dapp_id)
    }

    /// Returns the dapp id if the dapp has been added to the profile.
    pub fn fetch_id_for_profile(
        conn: &mut SqliteConnection,
        url: Url,
        public_suffix_list: &PublicSuffixList,
        profile_id: &DeterministicId,
    ) -> Result<Option<DeterministicId>, Error> {
        let dapp_entity = DappEntity::new(url, public_suffix_list)?;
        dapp_entity.fetch_id_for_profile(conn, profile_id)
    }
}

#[derive(Insertable)]
#[diesel(table_name = dapps)]
struct DappEntity {
    identifier: String,
    url: UrlValue,
}

impl DappEntity {
    fn new(url: Url, public_suffix_list: &PublicSuffixList) -> Result<Self, Error> {
        let origin = url.origin();
        let registrable_domain: Option<String> =
            public_suffix_list.registrable_domain(&origin)?.into();
        let identifier =
            registrable_domain.unwrap_or_else(|| origin.ascii_serialization());
        Ok(DappEntity {
            identifier,
            url: url.into(),
        })
    }

    /// Returns the dapp id if the dapp has been added to the profile.
    fn fetch_id_for_profile(
        &self,
        conn: &mut SqliteConnection,
        profile_id: &DeterministicId,
    ) -> Result<Option<DeterministicId>, Error> {
        use asymmetric_keys::dsl as ak;
        use dapps::dsl as d;
        use diesel::{expression::AsExpression, sql_types::Bool};
        use profiles::dsl as p;

        let deterministic_id = self.deterministic_id()?;

        let maybe_exists: Option<bool> = asymmetric_keys::table
            .inner_join(profiles::table.on(ak::profile_id.eq(p::deterministic_id)))
            .inner_join(dapps::table.on(ak::dapp_id.eq(d::deterministic_id.nullable())))
            .filter(p::deterministic_id.eq(profile_id))
            .filter(d::deterministic_id.eq(&deterministic_id))
            // `exists` query is unstable
            .select(AsExpression::<Bool>::as_expression(true))
            .first(conn)
            .optional()?;

        match maybe_exists {
            Some(exists) if exists => Ok(Some(deterministic_id)),
            _ => Ok(None),
        }
    }

    /// Create a dapp entity and return its deterministic id.
    /// The operation is idempotent.
    fn create_if_not_exists(
        &self,
        conn: &mut SqliteConnection,
    ) -> Result<DeterministicId, Error> {
        use dapps::dsl as d;

        let deterministic_id = self.deterministic_id()?;
        let created_at = rfc3339_timestamp();

        diesel::insert_into(dapps::table)
            .values((
                self,
                d::deterministic_id.eq(&deterministic_id),
                d::created_at.eq(&created_at),
            ))
            .on_conflict_do_nothing()
            .execute(conn)?;

        Ok(deterministic_id)
    }
}

impl<'a> DeriveDeterministicId<'a, &'a str, U1> for DappEntity {
    fn entity_name(&'a self) -> EntityName {
        EntityName::Dapp
    }

    fn unique_columns(&'a self) -> GenericArray<&'a str, U1> {
        let identifier = self.identifier.as_str();
        [identifier].into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dapp_identifier() {
        let psl: PublicSuffixList = Default::default();

        let url = Url::parse("https://www.example.com").unwrap();
        let identifier = Dapp::dapp_identifier(url, &psl).unwrap();
        assert_eq!(identifier, "example.com");
    }
}
