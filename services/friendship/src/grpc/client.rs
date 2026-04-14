use relay_proto::identity::identity_service_client::IdentityServiceClient;
use relay_proto::identity::{GetUsersByIdsRequest, GetUsersByIdsResponse};
use tonic::metadata::MetadataValue;
use tonic::transport::{Channel, Endpoint};
use tonic::{Request, Status};
use uuid::Uuid;

use super::ACTOR_USER_ID_METADATA;

#[derive(Clone)]
pub struct IdentityClient {
    client: IdentityServiceClient<Channel>,
}

impl IdentityClient {
    pub async fn connect(
        dst: impl AsRef<str>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let channel = Endpoint::from_shared(dst.as_ref().to_string())?
            .connect()
            .await?;

        Ok(Self::new(channel))
    }

    pub fn new(channel: Channel) -> Self {
        Self {
            client: IdentityServiceClient::new(channel),
        }
    }

    pub async fn get_users_by_ids(
        &self,
        actor_user_id: Uuid,
        user_ids: Vec<Uuid>,
    ) -> Result<GetUsersByIdsResponse, Status> {
        let mut client = self.client.clone();
        let mut request = Request::new(GetUsersByIdsRequest {
            user_ids: user_ids
                .into_iter()
                .map(|user_id| user_id.to_string())
                .collect(),
        });
        request.metadata_mut().insert(
            ACTOR_USER_ID_METADATA,
            MetadataValue::try_from(actor_user_id.to_string())
                .expect("uuid string should be valid metadata value"),
        );

        Ok(client.get_users_by_ids(request).await?.into_inner())
    }

    pub async fn user_exists(&self, actor_user_id: Uuid, user_id: Uuid) -> Result<bool, Status> {
        let response = self.get_users_by_ids(actor_user_id, vec![user_id]).await?;
        Ok(!response.users.is_empty())
    }
}
