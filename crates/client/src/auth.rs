use crate::proto::{AuthConfig, MidenFalconRpoAuth};

pub fn miden_falcon_rpo_auth(cosigner_pubkeys: Vec<String>) -> AuthConfig {
    AuthConfig {
        auth_type: Some(crate::proto::auth_config::AuthType::MidenFalconRpo(
            MidenFalconRpoAuth { cosigner_pubkeys },
        )),
    }
}
