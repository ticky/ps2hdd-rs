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
        match unsafe { std::ffi::CStr::from_ptr(libc::strerror(-result)) }.to_str() {
            Ok(err) => return Err(format!("{}: {}, {}", err_message, result, err)),
            Err(error) => {
                return Err(format!(
                    "could not convert strerror message a String: {}",
                    error
                ))
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn return_ok_on_zeroes() {
        assert_eq!(
            ok_on_zero_or_strerror(0, "This message should not be returned"),
            Ok(0)
        );

        assert_eq!(
            ok_on_nonnegative_or_strerror(0, "This message should not be returned"),
            Ok(0)
        );
    }

    #[test]
    fn return_err_on_positives() {
        assert_eq!(
            ok_on_zero_or_strerror(-libc::EPERM, "This message should be returned"),
            Err("This message should be returned: -1, Operation not permitted".to_string())
        );

        assert_eq!(
            ok_on_zero_or_strerror(-libc::EACCES, "This message should be returned"),
            Err("This message should be returned: -13, Permission denied".to_string())
        );
    }

    #[test]
    fn return_ok_on_positives() {
        assert_eq!(
            ok_on_nonnegative_or_strerror(10, "This message should not be returned"),
            Ok(10)
        );

        assert_eq!(
            ok_on_nonnegative_or_strerror(123, "This message should not be returned"),
            Ok(123)
        );
    }

    #[test]
    fn return_err_on_negatives() {
        assert_eq!(
            ok_on_zero_or_strerror(-libc::EPERM, "This message should be returned"),
            Err("This message should be returned: -1, Operation not permitted".to_string())
        );

        assert_eq!(
            ok_on_zero_or_strerror(-libc::EACCES, "This message should be returned"),
            Err("This message should be returned: -13, Permission denied".to_string())
        );

        assert_eq!(
            ok_on_nonnegative_or_strerror(-libc::EIO, "This message should be returned"),
            Err("This message should be returned: -5, Input/output error".to_string())
        );

        assert_eq!(
            ok_on_nonnegative_or_strerror(-libc::EBUSY, "This message should be returned"),
            Err("This message should be returned: -16, Resource busy".to_string())
        );
    }
}