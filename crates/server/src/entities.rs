pub mod account {
    use sea_orm::entity::prelude::*;
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "accounts")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: String,
        #[sea_orm(unique)]
        pub username: String,
        pub password_hash: String,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}
pub mod session {
    use sea_orm::entity::prelude::*;
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "sessions")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: String,
        pub owner: String,
        pub expires_at: i64,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}
pub mod document {
    use sea_orm::entity::prelude::*;
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "documents")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: String,
        pub owner: String,
        pub kind: String,
        pub ref_id: String,
        pub revision: i64,
        #[sea_orm(column_type = "Text")]
        pub payload: String,
        pub updated_at: String,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}
pub mod setting {
    use sea_orm::entity::prelude::*;
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "settings")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: String,
        pub revision: i64,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}
pub mod access_token {
    use sea_orm::entity::prelude::*;
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "access_tokens")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: String,
        pub owner: String,
        pub name: String,
        #[sea_orm(unique)]
        pub digest: String,
        pub created_at: i64,
        pub expires_at: i64,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}
pub mod workspace_membership {
    use sea_orm::entity::prelude::*;
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "workspace_memberships")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: String,
        pub owner_id: String,
        pub workspace_id: String,
        pub member_id: String,
        pub role: String,
        pub generation: String,
        pub revision: i64,
        pub created_at: i64,
        pub updated_at: i64,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}
pub mod workspace_invitation {
    use sea_orm::entity::prelude::*;
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "workspace_invitations")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: String,
        pub owner_id: String,
        pub workspace_id: String,
        pub token_digest: String,
        pub role: String,
        pub created_by: String,
        pub created_at: i64,
        pub expires_at: i64,
        pub consumed_by: Option<String>,
        pub revoked_at: Option<i64>,
        pub revision: i64,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}
pub mod workspace_audit_event {
    use sea_orm::entity::prelude::*;
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "workspace_audit_events")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: String,
        pub owner_id: String,
        pub workspace_id: String,
        pub actor_id: String,
        pub action: String,
        pub subject_id: Option<String>,
        pub result: String,
        pub created_at: i64,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}
