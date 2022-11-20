
pub trait LoggableErrorResult<T> {
    fn ok_or_log(self) -> Option<T>;
}

impl<T,E: std::fmt::Debug> LoggableErrorResult<T> for Result<T,E> {
    fn ok_or_log(self) -> Option<T> {
        match self  {
            Ok(obj) => Some(obj),
            Err(e) => {
                log::error!("{:?}", e);
                None
            }
        }
    }
}
