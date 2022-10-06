//! Generate Rust unit tests for canonical test vectors.

use quote::{format_ident, quote};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct Packet {
    #[serde(rename = "packet")]
    name: String,
    tests: Vec<TestVector>,
}

#[derive(Debug, Deserialize)]
struct TestVector {
    packed: String,
    unpacked: Value,
    packet: Option<String>,
}

// Convert a string of hexadecimal characters into a Rust vector of
// bytes.
//
// The string `"80038302"` becomes `vec![0x80, 0x03, 0x83, 0x02]`.
fn hexadecimal_to_vec(hex: &str) -> proc_macro2::TokenStream {
    assert!(hex.len() % 2 == 0, "Expects an even number of hex digits");
    let bytes = hex.as_bytes().chunks_exact(2).map(|chunk| {
        let number = format!("0x{}", std::str::from_utf8(chunk).unwrap());
        syn::parse_str::<syn::LitInt>(&number).unwrap()
    });

    quote! {
        vec![#(#bytes),*]
    }
}

fn generate_unit_tests(input: &str, packet_names: &[&str], module_name: &str) {
    eprintln!("Reading test vectors from {input}, will use {} packets", packet_names.len());

    let data = std::fs::read_to_string(input)
        .unwrap_or_else(|err| panic!("Could not read {input}: {err}"));
    let packets: Vec<Packet> = serde_json::from_str(&data).expect("Could not parse JSON");

    let mut tests = Vec::new();
    for packet in &packets {
        for (i, test_vector) in packet.tests.iter().enumerate() {
            let packet_name = test_vector.packet.as_deref().unwrap_or(packet.name.as_str());
            if !packet_names.contains(&packet_name) {
                eprintln!("Skipping packet {}", packet_name);
                continue;
            }
            let test_name =
                format_ident!("{}_vector_{}_0x{}", packet_name, i + 1, &test_vector.packed);
            let packed = hexadecimal_to_vec(&test_vector.packed);
            let packet_name = format_ident!("{}Packet", packet_name);

            let object = test_vector.unpacked.as_object().unwrap_or_else(|| {
                panic!("Expected test vector object, found: {}", test_vector.unpacked)
            });
            let assertions = object.iter().map(|(key, value)| {
                let getter = format_ident!("get_{key}");
                let value_u64 = value
                    .as_u64()
                    .unwrap_or_else(|| panic!("Expected u64 for {key:?} key, got {value}"));
                let value = proc_macro2::Literal::u64_unsuffixed(value_u64);
                quote! {
                    assert_eq!(actual.#getter(), #value);
                }
            });

            let module = format_ident!("{}", module_name);
            tests.push(quote! {
                #[test]
                fn #test_name() {
                    let packed = #packed;
                    let actual = #module::#packet_name::parse(&packed).unwrap();
                    #(#assertions)*
                }
            });
        }
    }

    let code = quote! {
        #(#tests)*
    };
    println!("{code}");
}

fn main() {
    let input_path = std::env::args().nth(1).expect("Need path to JSON file with test vectors");
    let module_name = std::env::args().nth(2).expect("Need name for the generated module");
    // TODO(mgeisler): remove the `packet_names` argument when we
    // support all canonical packets.
    generate_unit_tests(&input_path, &["Packet_Scalar_Field"], &module_name);
}
