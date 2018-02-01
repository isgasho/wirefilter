extern crate libc;
extern crate wirefilter;

mod strings;

use libc::size_t;
use std::cmp::max;
use std::fmt;
use std::net::IpAddr;
use std::str::FromStr;
use strings::{ExternallyAllocatedStr, RustAllocatedString};
use wirefilter::{Bytes, Context, Filter};
use wirefilter::lex::LexErrorKind;
use wirefilter::types::{LhsValue, Type};

pub struct ParseError<'a> {
    msg: String,
    span_start: size_t,
    span_len: size_t,
    input: &'a str,
}

impl<'a> ParseError<'a> {
    pub fn new(input: &'a str, (err, span): (LexErrorKind, &str)) -> Self {
        ParseError {
            msg: err.to_string(),
            span_start: span.as_ptr() as usize - input.as_ptr() as usize,
            span_len: span.len(),
            input,
        }
    }
}

impl<'a> fmt::Display for ParseError<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Filter parsing error:")?;
        writeln!(f, "`{}`", self.input)?;

        for _ in 0..1 + self.span_start {
            write!(f, " ")?;
        }

        for _ in 0..max(1, self.span_len) {
            write!(f, "^")?;
        }

        writeln!(f, " {}", self.msg)
    }
}

#[repr(u8)]
pub enum ParsingResult<'a> {
    Err(RustAllocatedString),
    Ok(*mut Filter<'a>),
}

impl<'a> From<Filter<'a>> for ParsingResult<'a> {
    fn from(filter: Filter<'a>) -> Self {
        ParsingResult::Ok(Box::into_raw(Box::new(filter)))
    }
}

impl<'a, 'b> From<ParseError<'b>> for ParsingResult<'a> {
    fn from(err: ParseError<'b>) -> Self {
        let msg = RustAllocatedString::from(err.to_string());

        ParsingResult::Err(msg)
    }
}

impl<'a> Drop for ParsingResult<'a> {
    fn drop(&mut self) {
        if let ParsingResult::Ok(filter) = *self {
            drop(unsafe { Box::from_raw(filter) });
        }
    }
}

pub type Scheme = Context<String, Type>;

#[no_mangle]
pub unsafe extern "C" fn wirefilter_create_scheme() -> *mut Scheme {
    Box::into_raw(Box::new(Context::default()))
}

#[no_mangle]
pub extern "C" fn wirefilter_free_scheme(scheme: &mut Scheme) {
    drop(unsafe { Box::from_raw(scheme) });
}

#[no_mangle]
pub extern "C" fn wirefilter_add_unsigned_type_field_to_scheme<'a>(
    scheme: &mut Scheme,
    name: ExternallyAllocatedStr<'a>,
) {
    scheme.insert(name.to_string(), Type::Unsigned);
}

#[no_mangle]
pub extern "C" fn wirefilter_add_ip_type_field_to_scheme<'a>(
    scheme: &mut Scheme,
    name: ExternallyAllocatedStr<'a>,
) {
    scheme.insert(name.to_string(), Type::Ip);
}

#[no_mangle]
pub extern "C" fn wirefilter_add_bytes_type_field_to_scheme<'a>(
    scheme: &mut Scheme,
    name: ExternallyAllocatedStr<'a>,
) {
    scheme.insert(name.to_string(), Type::Bytes);
}

#[no_mangle]
pub extern "C" fn wirefilter_free_parsing_result(result: ParsingResult) {
    drop(result);
}

#[no_mangle]
pub extern "C" fn wirefilter_parse_filter<'s, 'i>(
    scheme: &'s Scheme,
    input: ExternallyAllocatedStr<'i>,
) -> ParsingResult<'s> {
    let input = input.as_str();

    match scheme.parse(input) {
        Ok(filter) => ParsingResult::from(filter),
        Err(err) => ParsingResult::from(ParseError::new(input, err)),
    }
}

pub type ExecutionContext<'a> = Context<&'a str, LhsValue<'a>>;

#[no_mangle]
pub unsafe extern "C" fn wirefilter_create_execution_context<'a>() -> *mut ExecutionContext<'a> {
    Box::into_raw(Box::new(Context::default()))
}

#[no_mangle]
pub extern "C" fn wirefilter_free_execution_context(exec_context: &mut ExecutionContext) {
    drop(unsafe { Box::from_raw(exec_context) });
}

#[no_mangle]
pub extern "C" fn wirefilter_add_unsigned_value_to_execution_context<'a>(
    exec_context: &mut ExecutionContext<'a>,
    name: ExternallyAllocatedStr<'a>,
    value: u64,
) {
    exec_context.insert(name.as_str(), LhsValue::Unsigned(value));
}

#[no_mangle]
pub extern "C" fn wirefilter_add_bytes_value_to_execution_context<'a>(
    exec_context: &mut ExecutionContext<'a>,
    name: ExternallyAllocatedStr<'a>,
    value: ExternallyAllocatedStr<'a>,
) {
    let bytes = Bytes::from(value.as_str());
    exec_context.insert(name.as_str(), LhsValue::Bytes(bytes));
}

#[no_mangle]
pub extern "C" fn wirefilter_add_ip_value_to_execution_context<'a>(
    exec_context: &mut ExecutionContext<'a>,
    name: ExternallyAllocatedStr<'a>,
    value: ExternallyAllocatedStr<'a>,
) {
    if let Ok(ip) = IpAddr::from_str(value.as_str()) {
        exec_context.insert(name.as_str(), LhsValue::Ip(ip));
    }
}

#[no_mangle]
pub extern "C" fn wirefilter_match(filter: &Filter, exec_context: &ExecutionContext) -> bool {
    exec_context.execute(filter)
}

#[cfg(test)]
mod ffi_test {
    use super::*;
    use wirefilter::{Field, FilterOp};
    use wirefilter::op::{CombiningOp, OrderingOp};
    use wirefilter::types::RhsValue;

    fn test_with_scheme<F: Fn(&mut Scheme)>(test_fn: F) {
        let scheme = unsafe { &mut *wirefilter_create_scheme() };

        wirefilter_add_ip_type_field_to_scheme(scheme, ExternallyAllocatedStr::from("ip1"));
        wirefilter_add_ip_type_field_to_scheme(scheme, ExternallyAllocatedStr::from("ip2"));

        wirefilter_add_bytes_type_field_to_scheme(scheme, ExternallyAllocatedStr::from("str1"));
        wirefilter_add_bytes_type_field_to_scheme(scheme, ExternallyAllocatedStr::from("str2"));

        wirefilter_add_unsigned_type_field_to_scheme(scheme, ExternallyAllocatedStr::from("num1"));
        wirefilter_add_unsigned_type_field_to_scheme(scheme, ExternallyAllocatedStr::from("num2"));

        test_fn(scheme);

        wirefilter_free_scheme(scheme);
    }

    fn create_execution_context<'a>() -> &'a mut ExecutionContext<'a> {
        let exec_context = unsafe { &mut *wirefilter_create_execution_context() };

        wirefilter_add_ip_value_to_execution_context(
            exec_context,
            ExternallyAllocatedStr::from("ip1"),
            ExternallyAllocatedStr::from("127.0.0.1"),
        );

        wirefilter_add_ip_value_to_execution_context(
            exec_context,
            ExternallyAllocatedStr::from("ip2"),
            ExternallyAllocatedStr::from("192.168.0.1"),
        );

        wirefilter_add_bytes_value_to_execution_context(
            exec_context,
            ExternallyAllocatedStr::from("str1"),
            ExternallyAllocatedStr::from("Hey"),
        );

        wirefilter_add_bytes_value_to_execution_context(
            exec_context,
            ExternallyAllocatedStr::from("str2"),
            ExternallyAllocatedStr::from("yo123"),
        );

        wirefilter_add_unsigned_value_to_execution_context(
            exec_context,
            ExternallyAllocatedStr::from("num1"),
            42,
        );

        wirefilter_add_unsigned_value_to_execution_context(
            exec_context,
            ExternallyAllocatedStr::from("num2"),
            1337,
        );

        exec_context
    }

    fn test_with_filter<T: Fn(&Filter)>(input: &'static str, func: T) {
        test_with_scheme(|scheme| {
            let result = wirefilter_parse_filter(scheme, ExternallyAllocatedStr::from(input));

            match result {
                ParsingResult::Ok(filter) => func(unsafe { &*filter }),
                ParsingResult::Err(ref err) => panic!("{}", err.as_str()),
            }

            wirefilter_free_parsing_result(result);
        });
    }

    #[test]
    fn parse_error() {
        test_with_scheme(|scheme| {
            let src = r#"num1 == "abc""#;
            let result = wirefilter_parse_filter(scheme, ExternallyAllocatedStr::from(src));

            match result {
                ParsingResult::Ok(_) => panic!("Error expected"),
                ParsingResult::Err(ref err) => assert_eq!(
                    err.as_str(),
                    "Filter parsing error:\n`num1 == \"abc\"`\n         ^^^^^ expected digit\n"
                ),
            }

            wirefilter_free_parsing_result(result);
        });
    }

    #[test]
    fn parse_filter() {
        test_with_filter(r#"num1 > 3 && str2 == "abc""#, |filter| {
            assert_eq!(
                *filter,
                Filter::Combine(
                    CombiningOp::And,
                    vec![
                        Filter::Op(
                            Field::new("num1"),
                            FilterOp::Ordering(OrderingOp::GreaterThan, RhsValue::Unsigned(3)),
                        ),
                        Filter::Op(
                            Field::new("str2"),
                            FilterOp::Ordering(
                                OrderingOp::Equal,
                                RhsValue::Bytes(Bytes::from("abc")),
                            ),
                        ),
                    ]
                )
            );
        });
    }

    #[test]
    fn match_filter() {
        let exec_context = create_execution_context();

        test_with_filter(
            r#"num1 > 41 && num2 == 1337 && ip1 != 192.168.0.1 && str2 ~ "yo\d+""#,
            |filter| {
                assert!(wirefilter_match(filter, exec_context));
            },
        );

        test_with_filter(
            r#"ip2 == 192.168.0.1 && (str1 == "Hey" || str2 == "ya")"#,
            |filter| {
                assert!(wirefilter_match(filter, exec_context));
            },
        );

        test_with_filter("ip1 == 127.0.0.1 && ip2 == 127.0.0.2", |filter| {
            assert!(!wirefilter_match(filter, exec_context));
        });

        wirefilter_free_execution_context(exec_context);
    }

    #[test]
    #[should_panic(expected = "Could not find previously registered field num1")]
    fn panic_on_missing_value() {
        let exec_context = unsafe { &mut *wirefilter_create_execution_context() };

        test_with_filter("num1 == 42", |filter| {
            wirefilter_match(filter, exec_context);
        });
    }

    #[test]
    #[should_panic(expected="Field num1 was previously registered with type Unsigned but now contains Bytes")]
    fn panic_on_wrong_exec_context_type() {
        let exec_context = create_execution_context();

        wirefilter_add_bytes_value_to_execution_context(
            exec_context,
            ExternallyAllocatedStr::from("num1"),
            ExternallyAllocatedStr::from("Hey"),
        );

        test_with_filter("num1 == 42", |filter| {
            wirefilter_match(filter, exec_context);
        });
    }
}
