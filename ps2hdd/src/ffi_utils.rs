/// FFI utility function which converts the return value of a C function whose
/// expected result is `0` into a `Result` type to reduce code repetition.
///
/// If a negative return value is encountered, the result contains a combination
/// of the `err_message` provided and the `strerror` value for the result.
pub fn ok_on_zero_or_strerror(
    result: std::os::raw::c_int,
    err_message: &str,
) -> Result<std::os::raw::c_int, String> {
    ok_on_pred_or_strerror(result, err_message, |ret| ret != 0)
}

/// FFI utility function which converts the return value of a C function whose
/// expected result is positive into a `Result` type to reduce code repetition.
///
/// If a negative return value is encountered, the result contains a combination
/// of the `err_message` provided and the `strerror` value for the result.
pub fn ok_on_nonnegative_or_strerror(
    result: std::os::raw::c_int,
    err_message: &str,
) -> Result<std::os::raw::c_int, String> {
    ok_on_pred_or_strerror(result, err_message, |ret| ret < 0)
}

fn ok_on_pred_or_strerror<F>(
    result: std::os::raw::c_int,
    err_message: &str,
    f: F,
) -> Result<std::os::raw::c_int, String>
where
    F: Fn(std::os::raw::c_int) -> bool,
{
    if f(result) {
        let err = unsafe { std::ffi::CStr::from_ptr(libc::strerror(-result)) }
            .to_str()
            // TODO: Make this return a different err?
            .expect("could not convert strerror message a String");
        return Err(format!("{}: {}, {}", err_message, result, err));
    }

    Ok(result)
}
