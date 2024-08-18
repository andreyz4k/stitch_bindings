use ::stitch_core::*;
use clap::Parser;
use pyo3::prelude::*;
use pyo3::wrap_pyfunction;

/// todo add docstring
#[pyfunction(programs, tasks, weights, name_mapping, panic_loud, args)]
fn compress_backend(
    py: Python,
    programs: Vec<String>,
    tasks: Option<Vec<String>>,
    weights: Option<Vec<f32>>,
    name_mapping: Option<Vec<(String, String)>>,
    panic_loud: bool,
    args: String,
) -> PyResult<String> {
    // disable the printing of panics, so that the only panic we see is the one that gets passed along in an Exception to Python
    if !panic_loud {
        std::panic::set_hook(Box::new(|_| {}));
    }

    let cfg = match MultistepCompressionConfig::try_parse_from(
        format!("compress {args}").split_whitespace(),
    ) {
        Ok(cfg) => cfg,
        Err(e) => panic!("Error parsing arguments: {}", e),
    };

    // release the GIL and call compression
    let (_step_results, json_res) = py.allow_threads(|| {
        multistep_compression(&programs, tasks, weights, name_mapping, None, &cfg)
    });

    // return as something you could json.loads(out) from in python
    Ok(json_res.to_string())
}

use std::ffi::CStr;
use std::os::raw::{c_char, c_float};

#[no_mangle]
pub extern "C" fn compress_backend_c(
    programs: *const *const c_char,
    program_count: usize,
    tasks: *const *const c_char,
    task_count: usize,
    weights: *const c_float,
    weight_count: usize,
    name_mapping: *const *const c_char,
    name_mapping_count: usize,
    panic_loud: bool,
    args: *const c_char,
) -> *mut c_char {
    // Convert C types to Rust types
    let programs: Vec<String> = unsafe {
        std::slice::from_raw_parts(programs, program_count)
            .iter()
            .map(|&p| CStr::from_ptr(p).to_string_lossy().into_owned())
            .collect()
    };
    let tasks: Option<Vec<String>> = if task_count > 0 {
        Some(unsafe {
            std::slice::from_raw_parts(tasks, task_count)
                .iter()
                .map(|&t| CStr::from_ptr(t).to_string_lossy().into_owned())
                .collect()
        })
    } else {
        None
    };
    let weights: Option<Vec<f32>> = if weight_count > 0 {
        Some(unsafe {
            std::slice::from_raw_parts(weights, weight_count)
                .iter()
                .copied()
                .collect()
        })
    } else {
        None
    };
    let name_mapping: Option<Vec<(String, String)>> = if name_mapping_count > 0 {
        Some(unsafe {
            std::slice::from_raw_parts(name_mapping, name_mapping_count)
                .iter()
                .map(|&nm| {
                    let binding = CStr::from_ptr(nm).to_string_lossy();
                    let mut parts = binding.splitn(2, ',');
                    let from = parts.next().unwrap().to_string();
                    let to = parts.next().unwrap().to_string();
                    (from, to)
                })
                .collect()
        })
    } else {
        None
    };

    // disable the printing of panics, so that the only panic we see is the one that gets passed along in an Exception to Python
    if !panic_loud {
        std::panic::set_hook(Box::new(|_| {}));
    }

    let cfg = match MultistepCompressionConfig::try_parse_from(
        format!(
            "compress {}",
            unsafe { CStr::from_ptr(args) }.to_string_lossy()
        )
        .split_whitespace(),
    ) {
        Ok(cfg) => cfg,
        Err(e) => panic!("Error parsing arguments: {}", e),
    };

    // release the GIL and call compression
    let (_step_results, json_res) =
        multistep_compression(&programs, tasks, weights, name_mapping, None, &cfg);

    // return as something you could json.loads(out) from in python
    json_res.to_string().as_ptr() as *mut c_char
}

/// todo add docstring
#[pyfunction(programs, abstractions, panic_loud, args)]
fn rewrite_backend(
    py: Python,
    programs: Vec<String>,
    abstractions: Vec<&PyAny>,
    panic_loud: bool,
    args: String,
) -> PyResult<(Vec<String>, String)> {
    // disable the printing of panics, so that the only panic we see is the one that gets passed along in an Exception to Python
    if !panic_loud {
        std::panic::set_hook(Box::new(|_| {}));
    }

    let cfg = match MultistepCompressionConfig::try_parse_from(
        format!("compress {args}").split_whitespace(),
    ) {
        Ok(cfg) => cfg,
        Err(e) => panic!("Error parsing arguments: {}", e),
    };

    let abstractions = abstractions
        .iter()
        .map(|a| {
            let mut set = ExprSet::empty(Order::ChildFirst, false, false);
            let idx = set
                .parse_extend(&a.getattr("body")?.extract::<String>()?)
                .unwrap();
            Ok(Invention {
                body: ExprOwned::new(set, idx),
                arity: a.getattr("arity")?.extract::<usize>()?,
                name: a.getattr("name")?.extract::<String>()?,
            })
        })
        .collect::<PyResult<Vec<_>>>()?;

    // release the GIL and call rewriting
    let (rewritten, _step_results, json_res) =
        py.allow_threads(|| rewrite_with_inventions(&programs, &abstractions, &cfg));

    // return as something you could json.loads(out) from in python
    Ok((rewritten, json_res.to_string()))
}

/// A Python module implemented in Rust.
#[pymodule]
fn stitch_core(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(compress_backend, m)?)?;
    m.add_function(wrap_pyfunction!(rewrite_backend, m)?)?;
    Ok(())
}
