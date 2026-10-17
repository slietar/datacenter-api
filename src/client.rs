use crate::live_state::{Fallible, Reading};
use serde::Serialize;
use serde::ser::{SerializeStruct, Serializer};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Serialize)]
pub struct ClientError {
    message: Cow<'static, str>,
    reason: Cow<'static, str>,
}

impl ClientError {
    /// A `ClientError` that doesn't leak any detail of the underlying error
    /// to the client.
    pub fn unknown() -> Self {
        Self {
            message: "Unknown error".into(),
            reason: "unknown".into(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ClientResult<T> {
    Ok(T),
    Err(ClientError),
    Partial { error: ClientError, value: T },
}

impl<T> ClientResult<T> {
    /// Converts a [`Reading`] into a `ClientResult`, mapping the value with
    /// `f`. Error details are replaced by [`ClientError::unknown`] so they
    /// don't leak to the client.
    pub fn from_reading<V, E>(reading: &Reading<V, E>, f: impl FnOnce(&V) -> T) -> Self {
        match reading {
            Fallible::Ok(value) => ClientResult::Ok(f(value)),
            Fallible::Partial { value, .. } => ClientResult::Partial {
                error: ClientError::unknown(),
                value: f(value),
            },
            Fallible::Err(_) => ClientResult::Err(ClientError::unknown()),
        }
    }
}

impl<T: Serialize> Serialize for ClientResult<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut st = s.serialize_struct("ClientResult", 2)?;
        match self {
            ClientResult::Ok(v) => {
                st.serialize_field("ok", &true)?;
                st.serialize_field("value", v)?;
            }
            ClientResult::Err(e) => {
                st.serialize_field("ok", &false)?;
                st.serialize_field("error", e)?;
            }
            ClientResult::Partial { error, value } => {
                st.serialize_field("ok", &false)?;
                st.serialize_field("error", error)?;
                st.serialize_field("value", value)?;
            }
        }
        st.end()
    }
}

impl std::convert::From<&anyhow::Error> for ClientError {
    fn from(err: &anyhow::Error) -> Self {
        Self {
            message: err.to_string().into(),
            reason: err.root_cause().to_string().into(),
        }
    }
}

impl<T> std::convert::From<anyhow::Result<T>> for ClientResult<T> {
    fn from(result: anyhow::Result<T>) -> Self {
        match result {
            Ok(value) => ClientResult::Ok(value),
            Err(err) => ClientResult::Err(ClientError::from(&err)),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HostClientState {
    pub comin: Option<ClientResult<crate::comin::client::State>>,
    pub headscale: Option<ClientResult<Option<crate::headscale::client::NodeState>>>,
    pub ipmi: Option<ClientResult<crate::ipmi::client::IpmiState>>,
}

/// For use with serde's [serialize_with] attribute
pub fn ordered_map<S, K: Ord + Serialize, V: Serialize>(
    value: &HashMap<K, V>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let ordered: BTreeMap<_, _> = value.iter().collect();
    ordered.serialize(serializer)
}
