use std::str::{self, FromStr};
use nom::{hex_digit, oct_digit, digit, is_alphanumeric, alpha, line_ending, not_line_ending, multispace};
use std::num::ParseIntError;

// Copied and modified from rust-lang/rust/src/libcore/num/mod.rs
trait FromStrRadix: PartialOrd + Copy {
	type Err;
	fn from_str_radix(s: &str, radix: u32) -> Result<Self, Self::Err>;
}

macro_rules! doit {
	($($t:ty)*) => ($(impl FromStrRadix for $t {
		type Err = ParseIntError;
		fn from_str_radix(s: &str, r: u32) -> Result<$t, Self::Err> { Self::from_str_radix(s, r) }
	})*)
}
doit! { i8 i16 i32 i64 isize u8 u16 u32 u64 usize }

trait Labeled {
	fn add_label(&mut self, label: &str);
}

#[derive(PartialEq, Debug)]
pub struct BootInfo {
	reserve_info: Vec<ReserveInfo>,
	boot_cpuid: u32,
	root: Node,
}

#[derive(PartialEq, Debug)]
pub struct ReserveInfo {
	address: u64,
	size: u64,
	labels: Vec<String>,
}

impl Labeled for ReserveInfo {
	fn add_label(&mut self, label: &str) {
		let label = label.to_string();
		if !self.labels.contains(&label) {
			self.labels.push(label);
		}
	}
}

#[derive(PartialEq, Debug)]
pub struct Node {
	deleted: bool,
	name: String,
	proplist: Vec<Property>,
	children: Vec<Node>,

	//fullpath: Option<PathBuf>,
	//length to the # part of node_name@#
	//basenamelen: usize,

	//phandle: u32,
	//addr_cells: i32,
	//size_cells: i32,

	labels: Vec<String>,
}

impl Labeled for Node {
	fn add_label(&mut self, label: &str) {
		let label = label.to_string();
		if !self.labels.contains(&label) {
			self.labels.push(label);
		}
	}
}

#[derive(PartialEq, Debug)]
pub struct Property {
	deleted: bool,
	name: String,
	val: Option<Vec<Data>>,
	labels: Vec<String>,
}

impl Labeled for Property {
	fn add_label(&mut self, label: &str) {
		let label = label.to_string();
		if !self.labels.contains(&label) {
			self.labels.push(label);
		}
	}
}

impl Property {
	fn delete(&mut self) {
		self.deleted = true;
		self.labels.clear();
	}
}

#[derive(PartialEq, Debug)]
pub enum Data {
	Reference(String),
	String(String),
	Cells(Vec<(u64, Option<String>)>),
	ByteArray(Vec<u8>),
}

fn from_str_hex<T: FromStrRadix>(s: &str) -> Result<T ,T::Err> {
	T::from_str_radix(s, 16)
}

fn from_str_oct<T: FromStrRadix>(s: &str) -> Result<T ,T::Err> {
	T::from_str_radix(s, 8)
}
fn from_str_dec<T: FromStr>(s: &str) -> Result<T ,T::Err> {
	T::from_str(s)
}

/* 
 * This is dumb and feels wrong, but it works so I will complain no more.
 * Thank you to Filipe Gonçalves and https://crates.io/crates/config for the inspiration.
 */
named!(eat_junk, do_parse!(many0!(alt_complete!(
	delimited!(tag!("/*"), take_until!("*/"), tag!("*/")) |
	delimited!(tag!("//"), not_line_ending, line_ending) |
	multispace
)) >> (&b""[..])));

macro_rules! comments_ws (
	($i:expr, $($args:tt)*) => ( {
		use $crate::dts_parser::eat_junk;
		sep!($i, eat_junk, $($args)*)
	} )
);

//TODO: math operations
named!(num_u64<u64>, alt_complete!(
		preceded!(tag_no_case!("0x"), map_res!(map_res!(hex_digit, str::from_utf8), from_str_hex::<u64>)) |
		preceded!(tag_no_case!("0"), map_res!(map_res!(oct_digit, str::from_utf8), from_str_oct::<u64>)) |
		map_res!(map_res!(digit, str::from_utf8), from_str_dec::<u64>)
));

named!(num_u32<u32>, alt_complete!(
		preceded!(tag_no_case!("0x"), map_res!(map_res!(hex_digit, str::from_utf8), from_str_hex::<u32>)) |
		preceded!(tag_no_case!("0"), map_res!(map_res!(oct_digit, str::from_utf8), from_str_oct::<u32>)) |
		map_res!(map_res!(digit, str::from_utf8), from_str_dec::<u32>)
));

named!(num_u16<u16>, alt_complete!(
		preceded!(tag_no_case!("0x"), map_res!(map_res!(hex_digit, str::from_utf8), from_str_hex::<u16>)) |
		preceded!(tag_no_case!("0"), map_res!(map_res!(oct_digit, str::from_utf8), from_str_oct::<u16>)) |
		map_res!(map_res!(digit, str::from_utf8), from_str_dec::<u16>)
));

named!(num_u8<u8>, alt_complete!(
		preceded!(tag_no_case!("0x"), map_res!(map_res!(hex_digit, str::from_utf8), from_str_hex::<u8>)) |
		preceded!(tag_no_case!("0"), map_res!(map_res!(oct_digit, str::from_utf8), from_str_oct::<u8>)) |
		map_res!(map_res!(digit, str::from_utf8), from_str_dec::<u8>)
));

fn is_prop_node_char(c: u8) -> bool {
	is_alphanumeric(c) ||
	c == b',' || c == b'.' || c == b'_' ||
	c == b'+' || c == b'*' || c == b'#' ||
	c == b'?' || c == b'@' || c == b'-'
}

fn is_path_char(c: u8) -> bool {
	is_prop_node_char(c) || c == b'/'
}

fn is_label_char(c: u8) -> bool {
	is_alphanumeric(c) || c == b'_'
}

named!(parse_label<String>,
	map!(map_res!(
		recognize!(preceded!(alt!(alpha | tag!("_")), take_while!(is_label_char))),
	str::from_utf8), String::from)
);

named!(parse_ref<String>, preceded!(
	char!('&'),
	delimited!(
		char!('{'),
		map!(map_res!(take_while1!(is_path_char), str::from_utf8), String::from),
		char!('}')
	)
));

// Warning! Only supports hex escape codes up to 0x7f because UTF-8 reasons
named!(escape_c_string<String>, map_res!(
	escaped_transform!(take_until_either!("\\\""), '\\',
		alt!(
			tag!("a")	=> { |_| vec![b'\x07'] } |
			tag!("b")	=> { |_| vec![b'\x08'] } |
			tag!("t")	=> { |_| vec![b'\t'] } |
			tag!("n")	=> { |_| vec![b'\n'] } |
			tag!("v")	=> { |_| vec![b'\x0B'] } |
			tag!("f")	=> { |_| vec![b'\x0C'] } |
			tag!("r")	=> { |_| vec![b'\r'] } |
			tag!("\\")	=> { |_| vec![b'\\'] } |
			tag!("\"")	=> { |_| vec![b'\"'] } |
			preceded!(
				tag_no_case!("x"),
				map_res!(map_res!(hex_digit, str::from_utf8), from_str_hex::<u8>)
			) => { |c| vec![c] } |
			map_res!(map_res!(oct_digit, str::from_utf8), from_str_oct::<u8>) => { |c| vec![c] }
		)),
	String::from_utf8)
);

named!(parse_cell<(u64, Option<String>)>, do_parse!(
	num: opt!(num_u64) >>
	r: cond!(num.is_none(), parse_ref) >>
	(num.unwrap_or(0), r)
));

named!(parse_mem_reserve<ReserveInfo>, comments_ws!(do_parse!(
	labels: many0!(parse_label) >>
	tag!("/memreserve/") >>
	addr: num_u64 >>
	size: num_u64 >>
	char!(';') >>
	(ReserveInfo { address: addr, size: size, labels: labels })
)));

//TODO: labels in data
//TODO: include binary
named!(pub parse_data<Data>, comments_ws!(alt_complete!(
	delimited!(
		char!('"'),
		do_parse!(
			val: escape_c_string >>
			(Data::String(val))
		),
		char!('"')
	) |
	//TODO: sized cells
	//TODO: references, only in 32 bit ones
	delimited!(
		char!('<'),
		do_parse!(val: separated_nonempty_list!(char!(' '), parse_cell) >> (Data::Cells(val))),
		char!('>')
	) |
	delimited!(
		char!('['),
		do_parse!(
			val: many1!(map_res!(map_res!(comments_ws!(take!(2)), str::from_utf8), from_str_hex::<u8>)) >>
			(Data::ByteArray(val))
		),
		char!(']')
	) |
	do_parse!(val: parse_ref >> (Data::Reference(val)))
)));

named!(pub parse_prop<Property>, comments_ws!(do_parse!(
	labels: many0!(terminated!(parse_label, char!(':'))) >>
	name: tap!(res:
		map!(map_res!(take_while1!(is_prop_node_char), str::from_utf8), String::from) => {
			println!("found {:?}", res)
		}
	) >>
	data: tap!(res:
		opt!(preceded!(char!('='), separated_nonempty_list!(comments_ws!(char!(',')), parse_data))) => {
			println!("data {:?}", res)
		}
	) >>
	char!(';') >>
	(Property {deleted: false, name: name, val: data, labels: labels})
)));

named!(parse_node<Node>, comments_ws!(do_parse!(
	name: map!(map_res!(alt_complete!(
		take_while1!(is_prop_node_char) |
		tag!("/")
	), str::from_utf8), String::from) >>
	char!('{') >>
	props: many0!(parse_prop) >>
	subnodes: many0!(parse_node) >>
	char!('}') >>
	char!(';') >>
	(Node { name: name, deleted: false, proplist: props, children: subnodes, labels: Vec::new() })
)));

//TODO: stuff after main block
named!(parse_device_tree<Node>, comments_ws!(preceded!(peek!(char!('/')), parse_node)));

named!(parse_dts<BootInfo>, comments_ws!(do_parse!(
	tag!("/dts-v1/;") >>
	mem_reserves: many0!(parse_mem_reserve) >>
	device_tree: parse_device_tree >>
	(BootInfo { reserve_info: mem_reserves, root: device_tree, boot_cpuid: 0 })
)));

//TODO: imports
//TODO: delete nodes
//TODO: delete props
//TODO: cpp linemarkers
//TODO: error messages
pub fn parse_dt(source: &[u8]) {
	println!("{:#?}", parse_dts(source));
}

#[cfg(test)]
mod tests {
	use super::*;
	use nom::IResult;

	#[test]
	fn parse_prop_empty() {
		assert_eq!(
			parse_prop(b"empty_prop;"), 
			IResult::Done(
				&b""[..],
				Property {
					deleted: false,
					name: "empty_prop".to_string(),
					val: None,
					labels: Vec::new(),
				}
			)
		);
	}

	#[test]
	fn parse_prop_cells() {
		assert_eq!(
			parse_prop(b"cell_prop = < 1 2 10 >;"), 
			IResult::Done(
				&b""[..],
				Property {
					deleted: false,
					name: "cell_prop".to_string(),
					val: Some(vec![Data::Cells(vec![(1, None), (2, None), (10, None)])]),
					labels: Vec::new(),
				}
			)
		);
	}

	#[test]
	fn parse_prop_strings() {
		assert_eq!(
			parse_prop(b"string_prop = \"string\", \"string2\";"), 
			IResult::Done(
				&b""[..],
				Property {
					deleted: false,
					name: "string_prop".to_string(),
					val: Some(vec![Data::String("string".to_string()), Data::String("string2".to_string())]),
					labels: Vec::new(),
				}
			)
		);
	}

	#[test]
	fn parse_prop_bytes() {
		assert_eq!(
			parse_prop(b"bytes_prop = [1234 56 78];"), 
			IResult::Done(
				&b""[..],
				Property {
					deleted: false,
					name: "bytes_prop".to_string(),
					val: Some(vec![Data::ByteArray(vec![0x12, 0x34, 0x56, 0x78])]),
					labels: Vec::new(),
				}
			)
		);
	}

	#[test]
	fn parse_prop_mixed() {
		assert_eq!(
			parse_prop(b"mixed_prop = \"abc\", [1234], <0xa 0xb 0xc>;"), 
			IResult::Done(
				&b""[..],
				Property {
					deleted: false,
					name: "mixed_prop".to_string(),
					val: Some(vec![
						Data::String("abc".to_string()),
						Data::ByteArray(vec![0x12, 0x34]),
						Data::Cells(vec![(0xa, None), (0xb, None), (0xc, None)])
					]),
					labels: Vec::new(),
				}
			)
		);
	}

	#[test]
	fn block_comment() {
		assert_eq!(
			parse_prop(b"test_prop /**/ = < 1 2 10 >;"), 
			IResult::Done(
				&b""[..],
				Property {
					deleted: false,
					name: "test_prop".to_string(),
					val: Some(vec![Data::Cells(vec![(1, None), (2, None), (10, None)])]),
					labels: Vec::new(),
				}
			)
		);
	}

	#[test]
	fn line_comment() {
		assert_eq!(
			parse_prop(b"test_prop // stuff\n\t= < 1 2 10 >;"), 
			IResult::Done(
				&b""[..],
				Property {
					deleted: false,
					name: "test_prop".to_string(),
					val: Some(vec![Data::Cells(vec![(1, None), (2, None), (10, None)])]),
					labels: Vec::new(),
				}
			)
		);
	}

	#[test]
	fn parse_data_string() {
		assert_eq!(
			parse_data(b"\"\\x7f\\0stuffstuff\\t\\t\\t\\n\\n\\n\""), 
			IResult::Done(
				&b""[..],
				Data::String("\x7f\0stuffstuff\t\t\t\n\n\n".to_string())
			)
		);
	}
}