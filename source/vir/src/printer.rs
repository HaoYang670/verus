use crate::ast::*;
use air::printer::macro_push_node;
use air::{node, nodes};
use sise::Node;

const VIR_BREAK_ON: &[&str] = &["Function"];
const VIR_BREAK_AFTER: &[&str] =
    &["Block", ":variants", ":typ_params", ":typ_bounds", ":params", ":require", ":ensure"];

pub struct NodeWriter<'a> {
    pub break_on: std::collections::HashSet<&'a str>,
    pub break_after: std::collections::HashSet<&'a str>,
}

impl<'a> NodeWriter<'a> {
    pub(crate) fn new_vir() -> Self {
        use std::iter::FromIterator;
        NodeWriter {
            break_on: std::collections::HashSet::from_iter(VIR_BREAK_ON.iter().map(|x| *x)),
            break_after: std::collections::HashSet::from_iter(VIR_BREAK_AFTER.iter().map(|x| *x)),
        }
    }

    pub fn write_node(
        &mut self,
        writer: &mut sise::SpacedStringWriter,
        node: &Node,
        break_len: usize,
        brk: bool,
        brk_next: bool,
    ) {
        use sise::Writer;
        let opts =
            sise::SpacedStringWriterNodeOptions { break_line_len: if brk { 0 } else { break_len } };
        match node {
            Node::Atom(a) => {
                writer.write_atom(a, opts).unwrap();
            }
            Node::List(l) => {
                writer.begin_list(opts).unwrap();
                let mut brk = false;
                let brk_from_next = brk_next;
                let mut brk_next = false;
                for n in l {
                    self.write_node(writer, n, break_len + 1, brk || brk_from_next, brk_next);
                    brk_next = false;
                    brk = false;
                    if let Node::Atom(a) = n {
                        if self.break_on.contains(a.as_str()) {
                            brk = true;
                        }
                        if self.break_after.contains(a.as_str()) {
                            brk_next = true;
                        }
                    }
                }
                writer.end_list(()).unwrap();
            }
        }
    }

    fn clean_up_lines(input: String, _indentation: &str) -> String {
        let lines: Vec<&str> = input.lines().collect();
        let mut result: String = "".to_string();
        let mut i = 0;
        while i < lines.len() {
            let mut line = lines[i].to_owned();
            while i + 1 < lines.len() && lines[i + 1].trim() == ")" {
                line = line + lines[i + 1].trim();
                i += 1;
            }
            result.push_str(&line);
            i += 1;
            if i < lines.len() {
                result.push_str("\n");
            }
        }
        result
    }

    pub fn node_to_string(&mut self, node: &Node) -> String {
        use sise::Writer;
        let indentation = " ";
        let style = sise::SpacedStringWriterStyle { line_break: &("\n".to_string()), indentation };
        let mut result = String::new();
        let mut string_writer = sise::SpacedStringWriter::new(style, &mut result);
        self.write_node(&mut string_writer, &node, 120, false, false);
        string_writer.finish(()).unwrap();
        // Clean up result:
        Self::clean_up_lines(result, indentation)
    }
}

#[derive(Debug)]
pub struct ToNodeOpts {
    pub no_span: bool,
    pub no_type: bool,
    pub no_fn_details: bool,
    pub no_encoding: bool,
}

pub const COMPACT_TONODEOPTS: ToNodeOpts =
    ToNodeOpts { no_span: true, no_type: true, no_fn_details: true, no_encoding: true };

impl Default for ToNodeOpts {
    fn default() -> Self {
        Self { no_span: false, no_type: false, no_fn_details: false, no_encoding: false }
    }
}

pub(crate) trait ToNode {
    fn to_node(&self, opts: &ToNodeOpts) -> Node;
}

impl<A: ToNode> ToNode for crate::def::Spanned<A> {
    fn to_node(&self, opts: &ToNodeOpts) -> Node {
        if opts.no_span {
            self.x.to_node(opts)
        } else {
            Node::List(vec![
                Node::Atom("@".to_string()),
                Node::Atom(format!("\"{}\"", self.span.as_string)),
                self.x.to_node(opts),
            ])
        }
    }
}

impl<A: ToNode> ToNode for Vec<A> {
    fn to_node(&self, opts: &ToNodeOpts) -> Node {
        let nodes = self.iter().map(|x| x.to_node(opts)).collect();
        Node::List(nodes)
    }
}

impl<A: ToNode> ToNode for std::sync::Arc<A> {
    fn to_node(&self, opts: &ToNodeOpts) -> Node {
        (**self).to_node(opts)
    }
}

impl ToNode for String {
    fn to_node(&self, _opts: &ToNodeOpts) -> Node {
        Node::Atom(match self.is_ascii() {
            true => format!("\"{}\"", self),
            false => "non_ascii_string".to_string(),
        })
    }
}

impl<A: ToNode> ToNode for Option<A> {
    fn to_node(&self, opts: &ToNodeOpts) -> Node {
        match self {
            Some(v) => v.to_node(opts),
            None => Node::Atom("None".to_string()),
        }
    }
}

impl<A: ToNode, B: ToNode> ToNode for (A, B) {
    fn to_node(&self, opts: &ToNodeOpts) -> Node {
        let (a, b) = self;
        Node::List(vec![Node::Atom("tuple".to_string()), a.to_node(opts), b.to_node(opts)])
    }
}

impl<A: ToNode, B: ToNode, C: ToNode> ToNode for (A, B, C) {
    fn to_node(&self, opts: &ToNodeOpts) -> Node {
        let (a, b, c) = self;
        Node::List(vec![
            Node::Atom("tuple".to_string()),
            a.to_node(opts),
            b.to_node(opts),
            c.to_node(opts),
        ])
    }
}

impl ToNode for bool {
    fn to_node(&self, _opts: &ToNodeOpts) -> Node {
        Node::Atom(format!("{:?}", self))
    }
}

impl ToNode for u32 {
    fn to_node(&self, _opts: &ToNodeOpts) -> Node {
        Node::Atom(self.to_string())
    }
}

impl ToNode for char {
    fn to_node(&self, _opts: &ToNodeOpts) -> Node {
        let a = match self.is_ascii() {
            true => format!("'{}'", self.to_string()),
            false => format!("char<{:x}>", *self as u32),
        };
        Node::Atom(a)
    }
}

impl ToNode for u64 {
    fn to_node(&self, _opts: &ToNodeOpts) -> Node {
        Node::Atom(self.to_string())
    }
}

impl ToNode for usize {
    fn to_node(&self, _opts: &ToNodeOpts) -> Node {
        Node::Atom(self.to_string())
    }
}

impl ToNode for num_bigint::BigInt {
    fn to_node(&self, _opts: &ToNodeOpts) -> Node {
        Node::Atom(self.to_string())
    }
}

impl ToNode for air::ast::TypX {
    fn to_node(&self, opts: &ToNodeOpts) -> Node {
        use air::ast::TypX::*;
        match self {
            Bool => Node::Atom("Bool".to_string()),
            Int => Node::Atom("Int".to_string()),
            Lambda => Node::Atom("Lambda".to_string()),
            Named(ident) => Node::List(vec![Node::Atom("Named".to_string()), ident.to_node(opts)]),
            BitVec(size) => Node::List(vec![Node::Atom("BitVec".to_string()), size.to_node(opts)]),
        }
    }
}

impl<A: ToNode> ToNode for SpannedTyped<A> {
    fn to_node(&self, opts: &ToNodeOpts) -> Node {
        if opts.no_span && opts.no_type {
            self.x.to_node(opts)
        } else {
            let mut v = vec![Node::Atom("@@".to_string())];
            if !opts.no_span {
                v.push(Node::Atom(format!("\"{}\"", self.span.as_string)));
            }
            v.push(self.x.to_node(opts));
            if !opts.no_type {
                v.push(self.typ.to_node(opts));
            }
            Node::List(v)
        }
    }
}

impl<A: ToNode + Clone> ToNode for Binder<A> {
    fn to_node(&self, opts: &ToNodeOpts) -> Node {
        Node::List(vec![
            Node::Atom("->".to_string()),
            Node::Atom((**self.name).to_string()),
            self.a.to_node(opts),
        ])
    }
}

impl ToNode for Quant {
    fn to_node(&self, opts: &ToNodeOpts) -> Node {
        let Quant { quant, boxed_params } = self;
        let nodes = vec![
            Node::Atom(format!("{:?}", quant)),
            Node::Atom(":boxed_params".to_string()),
            boxed_params.to_node(opts),
        ];
        Node::List(nodes)
    }
}

impl ToNode for Mode {
    fn to_node(&self, _opts: &ToNodeOpts) -> Node {
        Node::Atom(format!("{:?}", self))
    }
}

impl ToNode for FunctionX {
    fn to_node(&self, opts: &ToNodeOpts) -> Node {
        if opts.no_fn_details {
            nodes!(
                Function
                {self.name.to_node(opts)}
                {Node::Atom(":mode".to_string())}
                {self.mode.to_node(opts)}
                {Node::Atom(":typ_bounds".to_string())}
                {self.typ_bounds.to_node(opts)}
                {Node::Atom(":params".to_string())}
                {self.params.to_node(opts)}
                {Node::Atom(":ret".to_string())}
                {self.ret.to_node(opts)}
                {Node::Atom(":require".to_string())}
                {self.require.to_node(opts)}
                {Node::Atom(":ensure".to_string())}
                {self.ensure.to_node(opts)}
                {Node::Atom(":body".to_string())}
                {self.body.to_node(opts)}
            )
        } else {
            self.to_node_inner(opts)
        }
    }
}

impl ToNode for ExprX {
    fn to_node(&self, opts: &ToNodeOpts) -> Node {
        if opts.no_encoding {
            match self {
                ExprX::Unary(UnaryOp::Clip { .. }, inner) => inner.to_node(opts),
                ExprX::UnaryOpr(UnaryOpr::Box(_) | UnaryOpr::Unbox(_), inner) => {
                    inner.to_node(opts)
                }
                _ => self.to_node_inner(opts),
            }
        } else {
            self.to_node_inner(opts)
        }
    }
}

fn path_to_node(path: &Path) -> Node {
    Node::Atom(format!(
        "\"{}\"",
        crate::def::path_to_string(path).replace("{", "_$LBRACE_").replace("}", "_$RBRACE_")
    ))
}

impl ToNode for Path {
    fn to_node(&self, _opts: &ToNodeOpts) -> Node {
        path_to_node(self)
    }
}

pub fn write_krate(mut write: impl std::io::Write, vir_crate: &Krate, opts: &ToNodeOpts) {
    let mut nw = NodeWriter::new_vir();

    let KrateX { datatypes, functions, traits, module_ids } = &**vir_crate;
    for datatype in datatypes.iter() {
        if opts.no_span {
            writeln!(&mut write, ";; {}", &datatype.span.as_string)
                .expect("cannot write to vir write");
        }
        writeln!(&mut write, "{}\n", nw.node_to_string(&datatype.to_node(opts)))
            .expect("cannot write to vir write");
    }
    for function in functions.iter() {
        if opts.no_span {
            writeln!(&mut write, ";; {}", &function.span.as_string)
                .expect("cannot write to vir write");
        }
        writeln!(&mut write, "{}\n", nw.node_to_string(&function.to_node(opts)))
            .expect("cannot write to vir write");
    }
    for t in traits.iter() {
        let t = nodes!(trait {path_to_node(&t.x.name)});
        writeln!(&mut write, "{}\n", nw.node_to_string(&t)).expect("cannot write to vir write");
    }
    for module_id in module_ids.iter() {
        let module_id_node = nodes!(module_id {path_to_node(module_id)});
        writeln!(&mut write, "{}\n", nw.node_to_string(&module_id_node))
            .expect("cannot write to vir write");
    }
}
