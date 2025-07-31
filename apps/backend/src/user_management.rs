use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait, ActiveModelTrait, Set};
use uuid::Uuid;
use chrono::Utc;
use crate::entity::users::{Entity as Users, Model as User, ActiveModel as UserActiveModel, Column};
use crate::jwt::Claims;

pub async fn ensure_user_exists(
    db: &DatabaseConnection,
    claims: &Claims,
) -> Result<User, sea_orm::DbErr> {
    // First, try to find user by external_id
    let existing_user = Users::find()
        .filter(Column::ExternalId.eq(&claims.sub))
        .one(db)
        .await?;

    match existing_user {
        Some(user) => {
            // User exists, return it
            Ok(user)
        }
        None => {
            // User doesn't exist, create a new one
            let new_user = UserActiveModel {
                id: Set(Uuid::new_v4()),
                external_id: Set(claims.sub.clone()),
                email: Set(claims.email.clone()),
                name: Set(None), // We can set this later if needed
                created_at: Set(Utc::now().into()),
                updated_at: Set(Utc::now().into()),
                ..Default::default()
            };

            let user = new_user.insert(db).await?;
            Ok(user)
        }
    }
} 