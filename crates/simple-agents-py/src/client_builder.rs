//! Python `ClientBuilder` — configures providers and healing for [`crate::Client`].
//!
//! The built [`crate::Client`] uses the first provider (OpenAI-compatible HTTP).

use crate::completion_helpers::py_err;
use crate::provider_helpers::build_provider_from_name;
use crate::Client;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use simple_agents_core::{HealingSettings, SimpleAgentsClient};
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;

fn healing_settings_from_py(obj: &Bound<'_, PyAny>) -> PyResult<HealingSettings> {
    let d = obj.downcast::<PyDict>()?;
    let enabled = d
        .get_item("enabled")?
        .and_then(|v| v.extract::<bool>().ok())
        .unwrap_or(true);
    let mut settings = if enabled {
        HealingSettings::default()
    } else {
        HealingSettings::disabled()
    };
    if let Some(mc) = d.get_item("min_confidence")? {
        if let Ok(v) = mc.extract::<f32>() {
            settings.parser_config.min_confidence = v;
            settings.coercion_config.min_confidence = v;
        }
    }
    if let Some(ft) = d.get_item("fuzzy_match_threshold")? {
        if let Ok(v) = ft.extract::<f64>() {
            settings.coercion_config.fuzzy_match_threshold = v;
        }
    }
    Ok(settings)
}

/// Provider entry used by [`ClientBuilder::add_provider_config`].
#[pyclass]
#[derive(Clone)]
pub struct ProviderConfig {
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    pub api_key: String,
    #[pyo3(get)]
    pub api_base: Option<String>,
}

#[pymethods]
impl ProviderConfig {
    #[new]
    #[pyo3(signature = (name, api_key, api_base=None))]
    fn new(name: String, api_key: String, api_base: Option<String>) -> Self {
        Self {
            name,
            api_key,
            api_base,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "ProviderConfig(name={:?}, api_key=<redacted>, api_base={:?})",
            self.name, self.api_base
        )
    }
}

#[pyclass]
pub struct ClientBuilder {
    providers: Vec<(String, Option<String>, Option<String>)>,
    healing: Option<PyObject>,
}

#[pymethods]
impl ClientBuilder {
    #[new]
    fn new() -> Self {
        Self {
            providers: Vec::new(),
            healing: None,
        }
    }

    #[pyo3(signature = (name, *, api_key=None, api_base=None, base_url=None))]
    fn add_provider<'a>(
        mut slf: PyRefMut<'a, Self>,
        name: &str,
        api_key: Option<&str>,
        api_base: Option<&str>,
        base_url: Option<&str>,
    ) -> PyResult<PyRefMut<'a, Self>> {
        let effective_base = api_base.or(base_url).map(|s| s.to_string());
        slf.providers.push((
            name.to_string(),
            api_key.map(|s| s.to_string()),
            effective_base,
        ));
        Ok(slf)
    }

    fn add_provider_config<'a>(
        mut slf: PyRefMut<'a, Self>,
        config: PyRef<'a, ProviderConfig>,
    ) -> PyResult<PyRefMut<'a, Self>> {
        slf.providers.push((
            config.name.clone(),
            Some(config.api_key.clone()),
            config.api_base.clone(),
        ));
        Ok(slf)
    }

    fn with_healing_config<'a>(
        mut slf: PyRefMut<'a, Self>,
        config: PyObject,
    ) -> PyResult<PyRefMut<'a, Self>> {
        slf.healing = Some(config);
        Ok(slf)
    }

    fn build(&self, py: Python<'_>) -> PyResult<Client> {
        if self.providers.is_empty() {
            return Err(PyRuntimeError::new_err("At least one provider is required"));
        }
        let (name, key, base) = &self.providers[0];
        let prov =
            build_provider_from_name(name.as_str(), key.as_deref(), base.as_deref(), None, None)
                .map_err(py_err)?;
        let healing = if let Some(ref h) = self.healing {
            let bound = h.bind(py);
            healing_settings_from_py(bound.as_any())?
        } else {
            HealingSettings::default()
        };
        let client = SimpleAgentsClient::with_healing(prov, healing);
        let runtime = Runtime::new().map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(Client::from_parts(Arc::new(Mutex::new(runtime)), client))
    }

    fn __repr__(&self) -> String {
        format!("ClientBuilder(providers={})", self.providers.len())
    }
}
