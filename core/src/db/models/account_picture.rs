// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::fmt::Debug;

use diesel::{prelude::*, SqliteConnection};
use generic_array::{typenum::U1, GenericArray};

use crate::{
    assets::load_profile_pic,
    db::{
        deterministic_id::{DeriveDeterministicId, DeterministicId, EntityName},
        schema::profile_pictures,
    },
    utils::{blake3_hash, rfc3339_timestamp},
    Error,
};

/// Deprecated in favor of ProfilePicture
#[deprecated]
#[derive(Clone, Debug, PartialEq, Eq, Queryable, Identifiable)]
#[diesel(primary_key(deterministic_id))]
#[diesel(table_name = profile_pictures)]
pub struct AccountPicture {
    pub deterministic_id: DeterministicId,
    pub image_name: Option<String>,
    pub image_hash: Vec<u8>,
    pub image: Vec<u8>,
    pub created_at: String,
    pub updated_at: Option<String>,
}

impl AccountPicture {
    pub fn fetch_image(conn: &mut SqliteConnection, id: &str) -> Result<Vec<u8>, Error> {
        use profile_pictures::dsl as ap;

        let image = profile_pictures::table
            .filter(ap::deterministic_id.eq(id))
            .select(ap::image)
            .first(conn)?;

        Ok(image)
    }

    /// Insert a bundled account picture into the database and return its deterministic id.
    pub fn insert_bundled(
        conn: &mut SqliteConnection,
        image_name: &str,
    ) -> Result<DeterministicId, Error> {
        let image = load_profile_pic(image_name)?;
        let image_hash = blake3_hash(&image);
        let entity = AccountPictureEntity {
            image_hash: image_hash.as_bytes(),
        };
        entity.create(conn, &image, Some(image_name))
    }
}

/// Deprecated in favor of ProfilePicture
#[deprecated]
#[derive(Insertable)]
#[diesel(table_name = profile_pictures)]
pub struct AccountPictureEntity<'a> {
    pub(crate) image_hash: &'a [u8],
}

impl<'a> AccountPictureEntity<'a> {
    /// Insert an account picture and return its deterministic id.
    fn create(
        &self,
        conn: &mut SqliteConnection,
        image: &[u8],
        image_name: Option<&str>,
    ) -> Result<DeterministicId, Error> {
        use profile_pictures::dsl as ap;

        let deterministic_id = self.deterministic_id()?;
        let created_at = rfc3339_timestamp();
        diesel::insert_into(profile_pictures::table)
            .values((
                self,
                ap::deterministic_id.eq(&deterministic_id),
                ap::image.eq(image),
                ap::image_name.eq(image_name),
                ap::created_at.eq(&created_at),
            ))
            .execute(conn)?;

        Ok(deterministic_id)
    }
}

impl<'a> DeriveDeterministicId<'a, &'a [u8], U1> for AccountPictureEntity<'a> {
    fn entity_name(&'a self) -> EntityName {
        EntityName::AccountPicture
    }

    fn unique_columns(&'a self) -> GenericArray<&'a [u8], U1> {
        [self.image_hash].into()
    }
}
