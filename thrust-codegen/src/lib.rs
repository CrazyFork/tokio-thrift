#![feature(question_mark)]

extern crate handlebars;
extern crate rustc_serialize;
use std::io::{self, Write};
use std::collections::BTreeMap;
use rustc_serialize::json::{self, Json, ToJson};
use handlebars::{Handlebars, RenderError, RenderContext, Helper, Context, JsonRender};
use thrust_parser::Ty;

// pub fn write_runner_match(wr: &mut Write, name: &str, method: &ServiceMethod) {
//     write!(wr, "\"{method}\" => {{\n", method=method.ident);
//     write!(wr, "let args: {service}_{method}_Args = try!(Deserialize::deserialize(de));\n", service=name, method=method.ident);
//     write!(wr, "let ret = self.service.{method}(", method=method.ident);

//     for arg in method.args.iter() {
//         write!(wr, "args.{},", arg.ident);
//     }

//     write!(wr, "
//     ).map(|val| {{
//         let mut buf = Vec::new();
//         {{
//             let mut s = BinarySerializer::new(&mut buf);

//             s.write_message_begin(\"{method}\", ThriftMessageType::Reply);
//             s.write_struct_begin(\"{method}_ret\");
//             s.write_field_begin(\"ret\", {ty}, 1);
//             val.serialize(&mut s);
//             s.write_field_stop();
//             s.write_field_end();
//             s.write_struct_end();
//             s.write_message_end();
//         }}
//         buf
//     }});
//     Ok(ret)", method=method.ident, ty=method.ty.to_protocol());

//     write!(wr, "\n}},");
// }

// pub fn write_runner_impl_begin(wr: &mut Write, name: &str) -> Result<(), Error> {
//     write!(wr, "
// impl<S> Runner for {name}Runner<S>
//     where S: {name}Service
// {{
//     fn run<D>(&mut self, de: &mut D, msg: ThriftMessage) -> Result<Future<Vec<u8>>, Error>
//         where D: Deserializer + ThriftDeserializer
//     {{
//         match &*msg.name {{
// ", name=name);
//     Ok(())
// }

// pub fn write_runner_impl_end(wr: &mut Write) {
//     write!(wr, "            _ => unimplemented!()
//         }}
//     }}
// }}");
// }

// pub fn write_runner(wr: &mut Write, name: &str) -> Result<(), Error> {
//     write!(wr, "
// pub struct {name}Runner<S: {name}Service> {{
//     service: S
// }}

// impl<S> {name}Runner<S> where S: {name}Service {{
//     pub fn new(service: S) -> {name}Runner<S> {{
//         {name}Runner {{
//             service: service
//         }}
//     }}
// }}", name=name);
//     Ok(())
// }

// pub fn write_server(wr: &mut Write, name: &str) -> Result<(), Error> {
//     write!(wr, "\n
// pub struct {name}Server {{ \
//     dispatcher: Sender<dispatcher::Incoming>,
//     pub handle: JoinHandle<ThrustResult<()>>,
// }}

// impl {name}Server {{
//     pub fn new<S>(service: S, addr: SocketAddr) -> {name}Server
//         where S: 'static + {name}Service
//     {{
//         use std::thread;
//         use std::sync::mpsc::channel;
//         use std::io::Cursor;

//         let (sender, receiver) = channel();
//         let (handle, tx) = Dispatcher::spawn(dispatcher::Role::Server(addr, sender)).unwrap();

//         let send_tx = tx.clone();
//         thread::spawn(move || {{
//             let mut runner = {name}Runner::new(service);
//             for (token, buf) in receiver.iter() {{
//                 let mut de = BinaryDeserializer::new(Cursor::new(buf));
//                 match de.read_message_begin() {{
//                     Ok(msg) => {{
//                         match runner.run(&mut de, msg) {{
//                             Ok(f) => {{
//                                 let chan = send_tx.clone();
//                                 f.and_then(move |buf| {{
//                                     chan.send(Incoming::Reply(token, buf));
//                                     Async::Ok(())
//                                 }});
//                             }},
//                             Err(err) => {{\n
//                             }}
//                         }}
//                     }},
//                     Err(err) => {{
//                         println!(\"[server]: error parsing thrift message: {{:?}}\", err);
//                     }}
//                 }}
//             }}
//         }});

//         {name}Server {{
//             dispatcher: tx,
//             handle: handle,
//         }}
//     }}
// }}", name=name);
//     Ok(())
// }

use thrust_parser::{
//    Struct,
    Namespace,
//    Enum,
//    Service,
//    ServiceMethod,
    Parser,
    Keyword,
//    StructField,
//    Ty
};

extern crate thrust_parser;

#[derive(Debug)]
pub enum Error {
    Other,
    IO(io::Error),
    Parser(thrust_parser::Error),
    Eof
}

impl From<io::Error> for Error {
    fn from(val: io::Error) -> Error {
        Error::IO(val)
    }
}

impl From<thrust_parser::Error> for Error {
    fn from(val: thrust_parser::Error) -> Error {
        Error::Parser(val)
    }
}

pub fn find_rust_namespace(parser: &mut Parser) -> Result<Namespace, Error> {
    loop {
        let ns = parser.parse_namespace()?;

        if &*ns.lang == "rust" {
            return Ok(ns);
        } else {
            continue;
        }
    }
}

// define a custom helper
fn helper_ty_to_protocol(_: &Context,
                 h: &Helper,
                 _: &Handlebars,
                 rc: &mut RenderContext)
                 -> Result<(), RenderError> {
    let param = try!(h.param(0).ok_or(RenderError::new("Param 0 is required for to_string helper.")));
    let rendered = param.value().render();
    let ty = Ty::from(rendered);
    let ret = ty.to_protocol();
    try!(rc.writer.write(ret.as_bytes()));
    Ok(())
}

fn helper_ty_expr(_: &Context,
                 h: &Helper,
                 _: &Handlebars,
                 rc: &mut RenderContext)
                 -> Result<(), RenderError> {
    let param = try!(h.param(0).ok_or(RenderError::new("Param 0 is required for to_string helper.")));
    let rendered = param.value().render();
    let ty = Ty::from(rendered);
    let expr = match ty {
        Ty::String => "de.deserialize_str()",
        Ty::I32 => "de.deserialize_i32()",
        Ty::I16 => "de.deserialize_i16()",
        Ty::I64 => "de.deserialize_i64()",
        _ => panic!("Unexpected type to deserialize_arg: {:?}.", ty)
    };
    try!(rc.writer.write(expr.as_bytes()));
    Ok(())
}



pub fn compile(parser: &mut Parser, wr: &mut Write) -> Result<(), Error> {
    let mut handlebars = Handlebars::new();
    for template in vec!["base", "service", "method"] {
        handlebars.register_template_file(template, format!("thrust-codegen/src/{}.hbs", template)).expect("failed to register template");
    }
    handlebars.register_helper("expr", Box::new(helper_ty_expr));
    handlebars.register_helper("to_protocol", Box::new(helper_ty_to_protocol));

    let mut data: BTreeMap<String, Json> = BTreeMap::new();

    try!(write!(wr, "{}", handlebars.render("base", &data).expect("faled to render base file")));


    loop {
        if parser.lookahead_keyword(Keyword::Enum) {
            parser.parse_enum()?;
        } else if parser.lookahead_keyword(Keyword::Struct) {
            parser.parse_struct()?;
        } else if parser.lookahead_keyword(Keyword::Service) {
            let service = parser.parse_service()?;
            data.insert("service".to_string(), json::encode(&service)
                        .ok()
                        .and_then(|s| Json::from_str(&s).ok())
                        .expect("internal error"));
            println!("{:?}", data);
            write!(wr, "{}", handlebars.render("service", &data).expect("internal error")).expect("faled to render service")
        } else {
            break;
        }
    }

    Ok(())
}

// pub struct ServiceCodegen;
// pub struct MethodCodegen;

/// ```notrust
/// Return type -> Future<$ty>
/// ```
// impl MethodCodegen {
//     pub fn build(wr: &mut Write, method: &ServiceMethod) -> Result<(), Error> {
//         write!(wr, "fn {}(&mut self", method.ident);

//         MethodCodegen::args(wr, &method.args)?;

//         write!(wr, ") ");
//         write!(wr, "{}", MethodCodegen::ret(&method.ty));
//         Ok(())
//     }

//     pub fn ret(val: &Ty) -> String {
//         format!("-> Future<{}>", val.to_string())
//     }

//     pub fn arg(wr: &mut Write, arg: &StructField) -> Result<(), Error> {
//         write!(wr, ", {}: {}", arg.ident, arg.ty.to_string());
//         Ok(())
//     }

//     pub fn args(wr: &mut Write, args: &Vec<StructField>) -> Result<(), Error> {
//         for arg in args {
//             MethodCodegen::arg(wr, arg)?;
//         }

//         Ok(())
//     }
// }

// fn ws(wr: &mut Write, n: usize) -> Result<(), Error> {
//     for i in 0..n {
//         write!(wr, "    ");
//     }

//     Ok(())
// }

// impl ServiceCodegen {
//     pub fn build(wr: &mut Write, service: &Service) -> Result<(), Error> {
//         ServiceCodegen::build_trait(wr, service)?;
//         ServiceCodegen::build_client_struct(wr, service)?;
//         ServiceCodegen::build_client_impl(wr, service)?;
//         ServiceCodegen::build_args_struct(wr, service)?;
//         ServiceCodegen::impl_serialize_args(wr, service)?;
//         ServiceCodegen::impl_deserialize_args(wr, service)?;
//         ServiceCodegen::impl_service_client(wr, service)?;

//         write_server(wr, &service.ident);
//         write_runner(wr, &service.ident);
//         write_runner_impl_begin(wr, &service.ident);

//         for method in service.methods.iter() {
//             write_runner_match(wr, &service.ident, method);
//         }

//         write_runner_impl_end(wr);

//         Ok(())
//     }

//     pub fn impl_service_client(wr: &mut Write, service: &Service) -> Result<(), Error> {
//         write!(wr, "\nimpl {}Service for {}Client {{\n", service.ident, service.ident);

//         for method in service.methods.iter() {
//             write!(wr, "\n");
//             ws(wr, 1);
//             MethodCodegen::build(wr, method)?;
//             write!(wr, " {{\n");

//             ws(wr, 2);
//             write!(wr, "use std::io::Cursor;\n");

//             ws(wr, 2);
//             write!(wr, "let (res, future) = Future::<(ThriftMessage, BinaryDeserializer<Cursor<Vec<u8>>>)>::channel();\n");

//             ws(wr, 2);
//             write!(wr, "let mut buf = Vec::new();\n");

//             ws(wr, 2);
//             write!(wr, "{{\n");

//             ws(wr, 3);
//             write!(wr, "let mut se = BinarySerializer::new(&mut buf);\n");

//             ws(wr, 3);
//             write!(wr, "se.write_message_begin(\"{method}\", ThriftMessageType::Call);\n", method=method.ident);

//             ws(wr, 3);
//             write!(wr, "let args = {}_{}_Args {{\n", service.ident, method.ident);

//             for arg in method.args.iter() {
//                 ws(wr, 4);
//                 write!(wr, "{}: {},\n", arg.ident, arg.ident);
//             }

//             ws(wr, 3);
//             write!(wr, "}};\n");

//             ws(wr, 3);
//             write!(wr, "args.serialize(&mut se);\n");

//             ws(wr, 3);
//             write!(wr, "se.write_message_end();\n");

//             ws(wr, 2);
//             write!(wr, "}}\n");

//             ws(wr, 2);
//             write!(wr, "self.dispatcher.send(Incoming::Call(\"{}\".to_string(), buf, Some(res))).unwrap();\n", method.ident);

//             ws(wr, 2);
//             write!(wr, "future.and_then(move |(msg, de)| {{\n");

//             ws(wr, 3);
//             write!(wr, "Async::Ok(\"foobar\".to_string())\n");

//             ws(wr, 2);
//             write!(wr, "}})\n");

//             ws(wr, 1);
//             write!(wr, "}}\n");
//         }

//         write!(wr, "}}\n");
//         Ok(())
//     }

//     pub fn impl_serialize_args(wr: &mut Write, service: &Service) -> Result<(), Error> {
//         for method in service.methods.iter() {
//             ServiceCodegen::impl_serialize_arg(wr, &service.ident, method)?;
//         }

//         Ok(())
//     }

//     pub fn serialize_arg(wr: &mut Write, arg: &StructField) -> Result<(), Error> {
//         ws(wr, 2);

//         write!(wr, "try!(s.write_field_begin(\"{}\", {}, {}));\n", arg.ident, arg.ty.to_protocol(), arg.seq);
//         ws(wr, 2);
//         write!(wr, "try!(self.{}.serialize(s));\n", arg.ident);
//         ws(wr, 2);
//         write!(wr, "try!(s.write_field_stop());\n");
//         ws(wr, 2);
//         write!(wr, "try!(s.write_field_end());\n");

//         Ok(())
//     }

//     pub fn deserialize_arg(wr: &mut Write, arg: &StructField) -> Result<(), Error> {

//         let expr = match arg.ty {
//             Ty::String => "de.deserialize_str()",
//             Ty::I32 => "de.deserialize_i32()",
//             Ty::I16 => "de.deserialize_i16()",
//             Ty::I64 => "de.deserialize_i64()",
//             _ => panic!("Unexpected type to deserialize_arg.")
//         };

//         ws(wr, 3);
//         write!(wr, "{}: {{\n", arg.ident);

//         ws(wr, 4);
//         write!(wr, "match try!(de.read_field_begin()).ty {{\n");

//         ws(wr, 5);
//         write!(wr, "ThriftType::Stop => {{ try!(de.read_field_begin()); }},\n");
//         ws(wr, 5);
//         write!(wr, "_ => {{}}\n");

//         ws(wr, 4);
//         write!(wr, "}}\n");

//         ws(wr, 4);
//         write!(wr, "let val = try!({});\n", expr);

//         ws(wr, 4);
//         write!(wr, "try!(de.read_field_end());\n");
//         ws(wr, 4);
//         write!(wr, "val\n");

//         ws(wr, 3);
//         write!(wr, "}},\n");

//         Ok(())
//     }

//     pub fn impl_deserialize_args(wr: &mut Write, service: &Service) -> Result<(), Error> {
//         for method in service.methods.iter() {
//             ServiceCodegen::impl_deserialize_arg(wr, &service.ident, method);
//         }
//         Ok(())
//     }

//     pub fn impl_deserialize_arg(wr: &mut Write, name: &str, method: &ServiceMethod) -> Result<(), Error> {
//         write!(wr, "\nimpl Deserialize for {}_{}_Args {{\n", name, method.ident);
//         ws(wr, 1);
//         write!(wr, "fn deserialize<D>(de: &mut D) -> Result<Self, Error>\n");
//         ws(wr, 1);
//         write!(wr, "  where D: Deserializer + ThriftDeserializer\n");
//         ws(wr, 1);
//         write!(wr, "{{\n");

//         ws(wr, 2);
//         write!(wr, "try!(de.read_struct_begin());\n");

//         ws(wr, 2);
//         write!(wr, "let args = {}_{}_Args {{\n", name, method.ident);

//         for arg in method.args.iter() {
//             ServiceCodegen::deserialize_arg(wr, arg)?;
//         }

//         ws(wr, 2);
//         write!(wr, "}};\n");

//         ws(wr, 2);
//         write!(wr, "try!(de.read_struct_end());\n");
//         ws(wr, 2);
//         write!(wr, "Ok(args)\n");

//         ws(wr, 1);
//         write!(wr, "}}");
//         write!(wr, "\n}}");
//         Ok(())
//     }

//     pub fn impl_serialize_arg(wr: &mut Write, name: &str, method: &ServiceMethod) -> Result<(), Error> {
//         write!(wr, "\nimpl Serialize for {}_{}_Args {{\n", name, method.ident);
//         ws(wr, 1);
//         write!(wr, "fn serialize<S>(&self, s: &mut S) -> Result<(), Error>\n");
//         ws(wr, 1);
//         write!(wr, "  where S: Serializer + ThriftSerializer\n");
//         ws(wr, 1);
//         write!(wr, "{{\n");

//         ws(wr, 2);
//         write!(wr, "try!(s.write_struct_begin(\"{}_{}_Args\"));\n", name, method.ident);

//         for arg in method.args.iter() {
//             ServiceCodegen::serialize_arg(wr, arg)?;
//         }

//         ws(wr, 2);
//         write!(wr, "try!(s.write_struct_end());\n");
//         ws(wr, 2);
//         write!(wr, "Ok(())\n");

//         ws(wr, 1);
//         write!(wr, "}}");
//         write!(wr, "\n}}");
//         Ok(())
//     }

//     pub fn build_args_struct(wr: &mut Write, service: &Service) -> Result<(), Error> {
//         for method in service.methods.iter() {
//             ServiceCodegen::build_arg_struct(wr, &service.ident, method)?;
//         }

//         Ok(())
//     }

//     pub fn build_arg_struct(wr: &mut Write, name: &str, method: &ServiceMethod) -> Result<(), Error> {
//         write!(wr, "\nstruct {}_{}_Args {{\n", name, method.ident);

//         for arg in method.args.iter() {
//             ws(wr, 1);
//             write!(wr, "{}: {},\n", arg.ident, arg.ty.to_string());
//         }

//         write!(wr, "}}\n");
//         Ok(())
//     }

//     pub fn build_client_service_impl(wr: &mut Write, service: &Service) -> Result<(), Error> {
//         write!(wr, "\n\n");
//         write!(wr, "impl {}Client {{\n", service.ident);
//         Ok(())
//     }

//     pub fn build_client_impl(wr: &mut Write, service: &Service) -> Result<(), Error> {
//         write!(wr, "\n\n");
//         write!(wr, "impl {}Client {{\n", service.ident);

//         ws(wr, 1);
//         write!(wr, "pub fn new(addr: SocketAddr) -> {}Client {{\n", service.ident);

//         ws(wr, 2);
//         write!(wr, "let (handle, tx) = Dispatcher::spawn(dispatcher::Role::Client(addr)).unwrap();\n");

//         write!(wr, "\n");
//         ws(wr, 2);
//         write!(wr, "{}Client {{\n", service.ident);

//         ws(wr, 3);
//         write!(wr, "dispatcher: tx,\n");

//         ws(wr, 3);
//         write!(wr, "handle: handle,\n");

//         ws(wr, 2);
//         write!(wr, "}}\n");

//         ws(wr, 1);
//         write!(wr, "}}\n");

//         write!(wr, "}}\n");
//         Ok(())
//     }

//     pub fn build_client_struct(wr: &mut Write, service: &Service) -> Result<(), Error> {
//         write!(wr, "\npub struct {}Client {{\n", service.ident);
//         ws(wr, 1);
//         write!(wr, "dispatcher: Sender<dispatcher::Incoming>,\n");
//         ws(wr, 1);
//         write!(wr, "pub handle: JoinHandle<ThrustResult<()>>,\n");
//         write!(wr, "}}\n");
//         Ok(())
//     }

//     pub fn build_trait(wr: &mut Write, service: &Service) -> Result<(), Error> {
//         write!(wr, "\npub trait {}Service: Send {{\n", service.ident)?;

//         for method in service.methods.iter() {
//             ws(wr, 1);
//             MethodCodegen::build(wr, method);
//             write!(wr, ";\n");
//         }

//         write!(wr, "}}\n")?;
//         Ok(())
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;
    use thrust_parser::{
        Ty,
        ServiceMethod,
        Service,
        Struct,
        Enum,
        Namespace,
        FieldAttribute,
        StructField
    };

    #[test]
    fn service_method_ret() {
        let method = ServiceMethod {
            ident: format!("Foobar"),
            ty: Ty::String,
            attr: FieldAttribute::Required,
            args: Vec::new()
        };

        let ret = MethodCodegen::ret(&method.ty);
        assert_eq!(&*ret, "-> Future<String>");
    }

    #[test]
    fn arg() {
        let mut buf = Vec::new();
        let arg = StructField {
            seq: 1,
            attr: FieldAttribute::Required,
            ty: Ty::I32,
            ident: "voodoo".to_string()
        };

        let ret = MethodCodegen::arg(&mut buf, &arg);
        assert_eq!(buf, b", voodoo: i32");
    }

    #[test]
    fn args() {
        let mut buf = Vec::new();
        let arg = StructField {
            seq: 1,
            attr: FieldAttribute::Required,
            ty: Ty::I32,
            ident: "voodoo".to_string()
        };

        let two = StructField {
            seq: 1,
            attr: FieldAttribute::Required,
            ty: Ty::String,
            ident: "sic".to_string()
        };

        let ret = MethodCodegen::args(&mut buf, &vec![arg, two]);
        assert_eq!(buf, b", voodoo: i32, sic: String");
    }

    #[test]
    fn service_method_build() {
        let mut buf = Vec::new();
        let arg = StructField {
            seq: 1,
            attr: FieldAttribute::Required,
            ty: Ty::I32,
            ident: "voodoo".to_string()
        };

        let ret = MethodCodegen::build(&mut buf, &ServiceMethod {
            ident: "query".to_string(),
            ty: Ty::String,
            attr: FieldAttribute::Required,
            args: vec![arg]
        });
        assert_eq!(&*String::from_utf8(buf).unwrap(), "fn query(&mut self, voodoo: i32) -> Future<String>;\n");
    }

    #[test]
    fn service_trait_build() {
        let mut buf = Vec::new();
        let arg = StructField {
            seq: 1,
            attr: FieldAttribute::Required,
            ty: Ty::I32,
            ident: "voodoo".to_string()
        };

        let service = Service {
            ident: "Flock".to_string(),
            methods: vec![ServiceMethod {
                ident: "query".to_string(),
                ty: Ty::String,
                attr: FieldAttribute::Required,
                args: vec![arg]
            }]
        };

        let ret = ServiceCodegen::build_trait(&mut buf, &service);
    }
}
