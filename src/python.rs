use std::borrow::Cow;
use std::collections::HashMap;

use pyo3::{prelude::*, py_run};

#[pyclass]
#[derive(Debug, Clone)]
pub struct PyRequest {
    #[pyo3(get, set)]
    pub(crate) verb: String,
    #[pyo3(get, set)]
    pub(crate) url: String,
    #[pyo3(get, set)]
    pub(crate) headers: HashMap<String, String>,
    #[pyo3(get, set)]
    pub(crate) query_params: Option<HashMap<String, String>>,
    #[pyo3(get, set)]
    pub(crate) body: Option<String>,
}

pub(crate) fn run_python_pre_script(
    python_script: &str,
    url: &str,
    verb: &str,
    headers: &HashMap<String, String>,
    query_params: Option<&HashMap<String, String>>,
    body: Option<Cow<'_, str>>,
) -> Result<PyRequest, PyErr> {
    Python::with_gil(|py| -> PyResult<PyRequest> {
        let r = PyRequest {
            verb: verb.to_string(),
            url: url.to_string(),
            headers: headers.to_owned(),
            query_params: query_params.map(|q| q.to_owned()),
            body: body.map(|b| b.into_owned()),
        };
        let request = PyCell::new(py, r)?;
        py_run!(py, request, python_script);
        Ok(request.extract::<PyRequest>()?)
    })
}
