//! Native/hosted capability disclosure for the dedicated classic Dubbo editor.
use crate::AppState;
use axum::{Json, extract::State};
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct Capability {
    available: bool,
    engine: &'static str,
}

pub(crate) async fn capability(State(state): State<AppState>) -> Json<Capability> {
    Json(Capability {
        available: state.dubbo_runtime.is_some()
            && state
                .project_runtime
                .java_path()
                .is_some_and(|path| path.is_file()),
        engine: "Apache Dubbo 3.3.6 classic/Hessian2",
    })
}
