//! Python `ClientBuilder` — configures providers and routing for [`crate::Client`].
//!
//! Multi-provider routing is recorded for diagnostics; the built [`crate::Client`] uses the
//! first provider (OpenAI-compatible HTTP), matching demo/test expectations.

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
    routing: Option<String>,
    cache_ttl: Option<u64>,
    healing: Option<PyObject>,
    #[allow(dead_code)]
    latency_cfg: Option<PyObject>,
    #[allow(dead_code)]
    fallback_cfg: Option<PyObject>,
    #[allow(dead_code)]
    cost_cfg: Option<PyObject>,
}

#[pymethods]
impl ClientBuilder {
    #[new]
    fn new() -> Self {
        Self {
            providers: Vec::new(),
            routing: None,
            cache_ttl: None,
            healing: None,
            latency_cfg: None,
            fallback_cfg: None,
            cost_cfg: None,
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

    fn with_routing<'a>(mut slf: PyRefMut<'a, Self>, mode: &str) -> PyResult<PyRefMut<'a, Self>> {
        let valid = ["direct", "round_robin", "latency", "cost", "fallback"];
        if !valid.contains(&mode) {
            return Err(PyRuntimeError::new_err(format!(
                "Unknown routing mode: {mode}"
            )));
        }
        slf.routing = Some(mode.to_string());
        Ok(slf)
    }

    fn with_latency_routing<'a>(
        mut slf: PyRefMut<'a, Self>,
        py: Python<'_>,
        config: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'a, Self>> {
        let d = config.downcast::<PyDict>()?;
        if let Some(a) = d.get_item("alpha")? {
            let alpha: f64 = a.extract()?;
            if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
                return Err(PyRuntimeError::new_err("alpha must be between 0.0 and 1.0"));
            }
        }
        slf.routing = Some("latency".to_string());
        slf.latency_cfg = Some(config.clone().into_py(py));
        Ok(slf)
    }

    fn with_cost_routing<'a>(
        mut slf: PyRefMut<'a, Self>,
        py: Python<'_>,
        config: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'a, Self>> {
        let d = config.downcast::<PyDict>()?;
        let pc = d
            .get_item("provider_costs")?
            .ok_or_else(|| PyRuntimeError::new_err("provider_costs is required"))?;
        let pc = pc.downcast::<PyDict>()?;
        for v in pc.values() {
            let cost: f64 = v.extract()?;
            if !cost.is_finite() || cost < 0.0 {
                return Err(PyRuntimeError::new_err("Invalid cost"));
            }
        }
        slf.routing = Some("cost".to_string());
        slf.cost_cfg = Some(config.clone().into_py(py));
        Ok(slf)
    }

    fn with_fallback_routing<'a>(
        mut slf: PyRefMut<'a, Self>,
        py: Python<'_>,
        config: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'a, Self>> {
        slf.routing = Some("fallback".to_string());
        slf.fallback_cfg = Some(config.clone().into_py(py));
        Ok(slf)
    }

    fn with_cache<'a>(
        mut slf: PyRefMut<'a, Self>,
        ttl_seconds: u64,
    ) -> PyResult<PyRefMut<'a, Self>> {
        slf.cache_ttl = Some(ttl_seconds);
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
        let prov = build_provider_from_name(name.as_str(), key.as_deref(), base.as_deref(), None)
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
        format!(
            "ClientBuilder(providers={}, routing={:?}, cache_ttl={:?})",
            self.providers.len(),
            self.routing,
            self.cache_ttl
        )
    }
}
