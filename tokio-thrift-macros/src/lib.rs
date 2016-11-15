#![crate_type="dylib"]
#![feature(plugin_registrar, quote, rustc_private, question_mark, concat_idents)]

extern crate tokio_thrift_parser;
extern crate syntax;
extern crate rustc;
extern crate rustc_plugin;

use tokio_thrift_parser::{Ast, Parser};
use syntax::util::small_vector::SmallVector;
use std::iter::Iterator;
use syntax::codemap::Span;
use syntax::fold::Folder;
use syntax::parse::{parser, token};
use syntax::print::pprust;
use syntax::ast::{self, TokenTree};
use syntax::ptr::P;
use syntax::ext::base::{ExtCtxt, MacResult, DummyResult, MacEager};
use rustc_plugin::Registry;

pub struct Compiler<'a: 'x, 'x> {
    inner: parser::Parser<'a>,
    cx: &'x mut ExtCtxt<'a>
}

impl<'a, 'x> Compiler<'a, 'x> {
    pub fn new(cx: &'x mut ExtCtxt<'a>, args: &[TokenTree]) -> Compiler<'a, 'x> {
        Compiler::<'a, 'x> {
            inner: cx.new_parser_from_tts::<'a>(args),
            cx: cx
        }
    }

    pub fn parse(&mut self) -> Option<String> {
        if let Ok(expr) = self.inner.parse_expr() {
            let entry = self.cx.expander().fold_expr(expr);
            let th = match entry.node {
                ast::ExprKind::Lit(ref lit) => {
                    match lit.node {
                        ast::LitKind::Str(ref s, _) => s.to_string(),
                        _ => {
                            self.cx.span_err(entry.span, &format!(
                             "expected string literal but got `{}`",
                             pprust::lit_to_string(&**lit)));
                            return None;
                        }
                    }
                },
                _ => {
                    self.cx.span_err(entry.span, &format!(
                    "expected string literal but got `{}`",
                    pprust::expr_to_string(&*entry)));
                    return None
                }
            };
            if !self.inner.eat(&token::Eof) {
                self.cx.span_err(self.inner.span, "only one string literal allowed");
                return None;
            }

            Some(th)
        } else {
            self.cx.parse_sess().span_diagnostic.err("failure parsing token tree");
            return None;
        }
    }

    pub fn code(&mut self, input: String) -> Result<P<ast::Item>, tokio_thrift_parser::Error> {
        let mut parser = Parser::new(&input);
        let mut items = Vec::new();

        // We expect a namespace to appear first.
        let ns = parser.parse_namespace()?;
        let module = token::str_to_ident(&ns.module);

        while let Ok(node) = parser.parse_item() {
            match node.ir(self.cx) {
                Some(item) => items.push(item),
                // The node didn't want to export an item. All good!
                None => {}
            }

            let v = node.second_ir(self.cx);

            for item in v.into_iter() {
                items.push(item);
            }
        }

        // pieces.push(parser.parse_enum()?.ir(self.cx));
        // pieces.push(parser.parse_struct()?.ir(self.cx));

        Ok(quote_item!(self.cx, pub mod $module {
            #![allow(dead_code, unused_imports)]
            use tokio_thrift::protocol::{Error, ThriftType};
            use tokio_thrift::{ThrustResult, ThrustError};
            use tokio_thrift::dispatcher::{self, Dispatcher, Incoming};
            use tokio_thrift::reactor::Message;
            use std::thread::JoinHandle;
            use std::net::SocketAddr;
            use std::sync::mpsc::{Sender, Receiver};
            use tangle::{Future, Async};
            use std::collections::{HashMap, HashSet};
            use tokio_thrift::protocol::{ThriftDeserializer, ThriftSerializer};
            use tokio_thrift::protocol::{Serializer, Deserializer};
            use tokio_thrift::protocol::{Deserialize, Serialize, ThriftMessage};
            use tokio_thrift::binary_protocol::{BinarySerializer, BinaryDeserializer};
            $items
        }).unwrap())
    }
}

fn expand_rn(cx: &mut ExtCtxt, sp: Span, args: &[TokenTree]) -> Box<MacResult + 'static> {
    if args.len() == 0 {
        // XXX: Return an empty future.
        return DummyResult::any(sp);
    }

    let mut compiler = Compiler::new(cx, args);
    let input = match compiler.parse() {
        Some(s) => s,
        None => panic!("Expected a string")
    };

    MacEager::items(SmallVector::one(compiler.code(input).unwrap()))
}

#[plugin_registrar]
pub fn plugin_registrar(reg: &mut Registry) {
    reg.register_macro("tokio_thrift", expand_rn);
}
