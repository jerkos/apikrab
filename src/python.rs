use std::borrow::Cow;
use std::collections::HashMap;

use pyo3::{
    prelude::*,
    types::{IntoPyDict, PyDict},
};

use crate::http::FetchResult;

#[pyclass]
#[derive(Debug, Clone)]
struct LoggingStdout {
    s: String,
}

#[pymethods]
impl LoggingStdout {
    fn write(&mut self, data: &str) {
        self.s.push_str(data);
    }
}

#[pyclass]
#[derive(Debug, Clone)]
pub struct PyRequest {
    #[pyo3(get, set)]
    pub(crate) verb: String,
    #[pyo3(get, set)]
    pub(crate) url: String,
    #[pyo3(get, set)]
    pub(crate) headers: Py<PyDict>,
    #[pyo3(get, set)]
    pub(crate) query_params: Option<Py<PyDict>>,
    #[pyo3(get, set)]
    pub(crate) body: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Request {
    pub(crate) verb: String,
    pub(crate) url: String,
    pub(crate) headers: HashMap<String, String>,
    pub(crate) query_params: Option<HashMap<String, String>>,
    pub(crate) body: Option<String>,
}

pub(crate) fn run_python_pre_script(
    python_script: &str,
    url: &str,
    verb: &str,
    headers: &HashMap<String, String>,
    query_params: Option<&HashMap<String, String>>,
    body: Option<Cow<'_, str>>,
) -> Result<(Request, String), PyErr> {
    Python::with_gil(|py| -> PyResult<(Request, String)> {
        let r = PyRequest {
            verb: verb.to_string(),
            url: url.to_string(),
            headers: headers.to_owned().into_py_dict(py).into(),
            query_params: query_params.map(|q| q.to_owned().into_py_dict(py).into()),
            body: body.map(|b| b.into_owned()),
        };
        // override sys.stdout to capture the output
        let log = LoggingStdout { s: String::new() };
        let sys = py.import("sys")?;
        sys.setattr("stdout", log.into_py(py))?;
        let locals = PyDict::new(py);
        locals.set_item("request", Py::new(py, r)?)?;
        py.run(python_script, None, Some(locals))?;
        // extract stdout
        let script_output = sys.getattr("stdout")?.extract::<LoggingStdout>()?;
        let modified_request = locals
            .get_item("request")?
            .unwrap()
            .extract::<PyRequest>()?;

        Ok((
            Request {
                verb: modified_request.verb,
                url: modified_request.url,
                headers: modified_request
                    .headers
                    .extract::<HashMap<String, String>>(py)?,
                query_params: modified_request
                    .query_params
                    .and_then(|q| q.extract::<HashMap<String, String>>(py).ok()),
                body: modified_request.body.map(|b| b.to_owned()),
            },
            script_output.s,
        ))
    })
}

#[pyclass]
pub struct PyFetchResult {
    #[pyo3(get, set)]
    pub(crate) headers: Py<PyDict>,
    #[pyo3(get, set)]
    pub(crate) body: String,
    #[pyo3(get, set)]
    pub(crate) status: u16,
}

pub(crate) fn run_python_post_script(
    python_script: &str,
    fetch_result: &FetchResult,
) -> PyResult<String> {
    Python::with_gil(|py| -> PyResult<String> {
        let r = PyFetchResult {
            headers: fetch_result.headers.to_owned().into_py_dict(py).into(),
            body: fetch_result.response.to_owned(),
            status: fetch_result.status,
        };
        // override sys.stdout to capture the output
        let log = LoggingStdout { s: String::new() };
        let sys = py.import("sys")?;
        sys.setattr("stdout", log.into_py(py))?;
        let locals = PyDict::new(py);
        locals.set_item("response", Py::new(py, r)?)?;
        py.run(python_script, None, Some(locals))?;
        // extract stdout
        let script_output = sys.getattr("stdout")?.extract::<LoggingStdout>()?;
        Ok(script_output.s)
    })
}
