use core::fmt;

use tree_sitter::Node;

macro_rules! define_ast_node {
    ($struct_ident: ident, [$($field_name: ident),*]) => {
        pub struct $struct_ident<'a> {
            source: &'a str,
            node: Node<'a>,
        }

        impl<'a> fmt::Debug for $struct_ident<'a> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($struct_ident))
                    $(
                        .field(stringify!($field_name), &self.$field_name())
                    )*
                    .finish()
            }
        }

        impl<'a> $struct_ident<'a> {
            pub fn new(source: &'a str, node: Node<'a>) -> Self {
                Self { source, node }
            }
        }
    };
}

macro_rules! define_ident_literal_from_first_child {
    ($ident_name: ident) => {
        pub fn $ident_name(&self) -> Option<&str> {
            self.node
                .named_child(0)
                .map(|node| node.utf8_text(self.source.as_bytes()).unwrap())
        }
    };
}

macro_rules! define_ident_literal_from_last_child {
    ($ident_name: ident) => {
        pub fn $ident_name(&self) -> Option<&str> {
            self.node
                .named_child(self.node.named_child_count() - 1)
                .map(|node| node.utf8_text(self.source.as_bytes()).unwrap())
        }
    };
}

macro_rules! define_named_ident_literal {
    ($ident_name: ident) => {
        pub fn $ident_name(&self) -> Option<&str> {
            self.node
                .child_by_field_name(stringify!($ident_name))
                .map(|node| node.utf8_text(self.source.as_bytes()).unwrap())
        }
    };
}

macro_rules! define_named_field {
    ($ident_name: ident, $ast_type: ident) => {
        pub fn $ident_name(&self) -> Option<$ast_type> {
            self.node
                .child_by_field_name(stringify!($ident_name))
                .map(|node| $ast_type::new(self.source, node))
        }
    };
}

macro_rules! define_field_from_first_child {
    ($ident_name: ident, $ast_type: ident) => {
        pub fn $ident_name(&self) -> Option<$ast_type> {
            self.node
                .named_child(0)
                .map(|node| $ast_type::new(self.source, node))
        }
    };
}

macro_rules! define_field_from_last_child {
    ($field_name: ident, $ast_type: ident) => {
        pub fn $field_name(&self) -> Option<$ast_type> {
            self.node
                .named_child(self.named_child_count() - 1)
                .map(|node| $ast_type::new(self.source, node))
        }
    };
}

macro_rules! define_proxy_array_named_field {
    ($field_name: ident, $ast_type: ident) => {
        pub fn $field_name(&self) -> Option<Vec<$ast_type>> {
            self.node
                .child_by_field_name(stringify!($field_name))
                .map(|node| {
                    let mut cursor = self.node.walk();
                    node.named_children(&mut cursor)
                        .map(|node| $ast_type::new(self.source, node))
                        .collect()
                })
        }
    };
}

macro_rules! define_type_field {
    ($field_name: ident) => {
        pub fn $field_name(&self) -> Option<Type> {
            self.node
                .child_by_field_name("type")
                .map(|node| Type::new(self.source, node))
        }
    };
}

macro_rules! define_enum {
    ($enum_name: ident, {$($node_ident: ident => $ast_type: ident),*}) => {
        #[derive(Debug)]
        pub enum $enum_name<'a> {
            $(
                $ast_type($ast_type<'a>),
            )*
        }

        impl<'a> $enum_name<'a> {
            pub fn new(source: &'a str, node: Node<'a>) -> Self {
                match node.kind() {
                    $(
                        stringify!($node_ident) => $enum_name::$ast_type($ast_type::new(source, node)),
                    )*
                    _ => unreachable!(),
                }
            }
        }
    }
}

define_ast_node!(SourceFile, [definition]);

impl<'a> SourceFile<'a> {
    pub fn definition(&self) -> Option<Definition> {
        self.node
            .named_child(0)
            .map(|node| Definition::new(self.source, node))
    }
}

#[derive(Debug)]
pub enum Definition<'a> {
    Script(ScriptBlock<'a>),
    Module(Module<'a>),
    ModuleAtAddress(AddressBlock<'a>),
}

impl<'a> Definition<'a> {
    pub fn new(source: &'a str, node: Node<'a>) -> Self {
        match node.kind() {
            "module_definition" => Definition::Module(Module::new(source, node)),
            "script_block" => Definition::Script(ScriptBlock::new(source, node)),
            _ => unreachable!(),
        }
    }
}

define_ast_node!(ScriptBlock, [func_def]);

impl<'a> ScriptBlock<'a> {
    define_field_from_first_child!(func_def, FuncDef);
}

define_ast_node!(AddressBlock, []);

define_ast_node!(Module, [name, body]);

impl<'a> Module<'a> {
    define_named_ident_literal!(name);
    define_proxy_array_named_field!(body, ModuleItem);
}

define_ast_node!(UseDecl, [address, module]);

impl<'a> UseDecl<'a> {
    define_named_ident_literal!(address);
    define_named_ident_literal!(module);
}

define_ast_node!(StructField, [field, typ]);

impl<'a> StructField<'a> {
    define_named_ident_literal!(field);
    define_type_field!(typ);
}

define_ast_node!(TypeParam, [name]);

impl<'a> TypeParam<'a> {
    define_ident_literal_from_first_child!(name);
}

define_ast_node!(StructDef, [name, type_parameters, fields]);

impl<'a> StructDef<'a> {
    define_named_ident_literal!(name);
    define_proxy_array_named_field!(type_parameters, TypeParam);
    define_proxy_array_named_field!(fields, StructField);
}

define_ast_node!(NativeStructDef, [name]);

impl<'a> NativeStructDef<'a> {
    define_named_ident_literal!(name);
}

#[derive(Debug)]
pub enum ModuleItem<'a> {
    Use(UseDecl<'a>),
    FuncDef(FuncDef<'a>),
    NativeFuncDef(NativeFuncDef<'a>),
    Struct(StructDef<'a>),
    NativeStruct(NativeStructDef<'a>),
}

impl<'a> ModuleItem<'a> {
    pub fn new(source: &'a str, node: Node<'a>) -> Self {
        match node.kind() {
            "use_decl" => ModuleItem::Use(UseDecl::new(source, node)),
            "usual_function_definition" => ModuleItem::FuncDef(FuncDef::new(source, node)),
            "native_function_definition" => {
                ModuleItem::NativeFuncDef(NativeFuncDef::new(source, node))
            }
            "struct_definition" => ModuleItem::Struct(StructDef::new(source, node)),
            "native_struct_definition" => {
                ModuleItem::NativeStruct(NativeStructDef::new(source, node))
            }
            _ => unreachable!(),
        }
    }
}

define_ast_node!(
    FuncDef,
    [name, type_parameters, params, return_type, acquires, body]
);

impl<'a> FuncDef<'a> {
    define_named_ident_literal!(name);
    define_proxy_array_named_field!(type_parameters, TypeParam);
    define_proxy_array_named_field!(params, FuncParam);
    define_named_field!(return_type, Type);
    define_proxy_array_named_field!(acquires, ModuleAccess);
    define_named_field!(body, Block);
}

define_ast_node!(NativeFuncDef, [name, type_parameters, params]);

impl<'a> NativeFuncDef<'a> {
    define_named_ident_literal!(name);
    define_proxy_array_named_field!(type_parameters, TypeParam);
    define_proxy_array_named_field!(params, FuncParam);
    define_proxy_array_named_field!(acquires, ModuleAccess);
}

define_ast_node!(FuncParam, [name, typ]);

impl<'a> FuncParam<'a> {
    define_named_ident_literal!(name);
    define_type_field!(typ);
}

define_ast_node!(Block, [items]);

impl<'a> Block<'a> {
    pub fn items(&self) -> Vec<BlockItem> {
        let mut cursor = self.node.walk();
        self.node
            .named_children(&mut cursor)
            .map(|node| BlockItem::new(self.source, node))
            .collect()
    }
}

#[derive(Debug)]
pub enum BlockItem<'a> {
    LetStatement(LetStatement<'a>),
    Expr(Expr<'a>),
}

impl<'a> BlockItem<'a> {
    pub fn new(source: &'a str, node: Node<'a>) -> Self {
        match node.kind() {
            "let_statement" => BlockItem::LetStatement(LetStatement::new(source, node)),
            _ => BlockItem::Expr(Expr::new(source, node)),
        }
    }
}

define_ast_node!(LetStatement, [binds, typ, exp]);

impl<'a> LetStatement<'a> {
    define_proxy_array_named_field!(binds, Bind);
    define_type_field!(typ);
    define_named_field!(exp, Expr);
}

// Binds
// **********************************************************************************
define_enum!(Bind, { bind_var => BindVar, bind_unpack => BindUnpack });

define_ast_node!(BindVar, [name]);

impl<'a> BindVar<'a> {
    define_ident_literal_from_first_child!(name);
}

define_ast_node!(BindUnpack, [module_access, bind_fields]);

impl<'a> BindUnpack<'a> {
    define_field_from_first_child!(module_access, ModuleAccess);
    define_proxy_array_named_field!(bind_fields, BindField);
}

define_ast_node!(BindField, [field, bind]);

impl<'a> BindField<'a> {
    define_named_ident_literal!(field);
    define_named_field!(bind, BindVar);
}

// Types
// **********************************************************************************
define_enum!(Type, { apply_type => ApplyType,
                     ref_type => RefType,
                     tuple_type => TupleType,
                     function_type => FunctionType });

define_ast_node!(ApplyType, [module_access, type_arguments]);

impl<'a> ApplyType<'a> {
    define_field_from_first_child!(module_access, ModuleAccess);
    define_proxy_array_named_field!(type_arguments, Type);
}

define_ast_node!(RefType, [typ]);

impl<'a> RefType<'a> {
    define_field_from_first_child!(typ, Type);
}

define_ast_node!(TupleType, [items]);

impl<'a> TupleType<'a> {
    pub fn items(&self) -> Vec<Type> {
        let mut cursor = self.node.walk();
        self.node
            .named_children(&mut cursor)
            .map(|node| Type::new(self.source, node))
            .collect()
    }
}

define_ast_node!(FunctionType, [param_types, return_type]);

impl<'a> FunctionType<'a> {
    define_proxy_array_named_field!(param_types, Type);
    define_named_field!(return_type, Type);
}

define_ast_node!(ModuleAccess, [address, module, name]);

impl<'a> ModuleAccess<'a> {
    define_named_ident_literal!(address);
    define_named_ident_literal!(module);
    define_ident_literal_from_last_child!(name);
}

// Expressions
// **********************************************************************************
#[derive(Debug)]
pub enum Expr<'a> {
    LambdaExpr(LambdaExpr<'a>),
    UnaryExpr(UnaryExpr<'a>),
}

impl<'a> Expr<'a> {
    pub fn new(source: &'a str, node: Node<'a>) -> Self {
        match node.kind() {
            "lambda_expression" => Expr::LambdaExpr(LambdaExpr::new(source, node)),
            _ => Expr::UnaryExpr(UnaryExpr::new(source, node)),
        }
    }
}

define_ast_node!(LambdaExpr, [bindings, exp]);

impl<'a> LambdaExpr<'a> {
    define_proxy_array_named_field!(bindings, Bind);
    define_named_field!(exp, Expr);
}

define_ast_node!(IfExpr, []);

impl<'a> IfExpr<'a> {
    define_proxy_array_named_field!(bindings, Bind);
    define_named_field!(exp, Expr);
}

// Unary Expression
// **********************************************************************************
#[derive(Debug)]
pub enum UnaryExpr<'a> {
    Term(Term<'a>),
}

impl<'a> UnaryExpr<'a> {
    pub fn new(source: &'a str, node: Node<'a>) -> Self {
        match node.kind() {
            _ => UnaryExpr::Term(Term::new(source, node)),
        }
    }
}

// Terminals
// **********************************************************************************
#[derive(Debug)]
pub enum Term<'a> {
    Literal(Literal<'a>),
}

impl<'a> Term<'a> {
    pub fn new(source: &'a str, node: Node<'a>) -> Self {
        match node.kind() {
            kind if kind.ends_with("literal") => Term::Literal(Literal::new(source, node)),
            _ => unreachable!(),
        }
    }
}

// Literals
// **********************************************************************************
#[derive(Debug)]
pub enum Literal<'a> {
    Address(&'a str),
    Num(&'a str),
    Bool(&'a str),
    ByteString(&'a str),
}

impl<'a> Literal<'a> {
    pub fn new(source: &'a str, node: Node<'a>) -> Self {
        let val = node.utf8_text(source.as_bytes()).unwrap();
        match node.kind() {
            "address_literal" => Literal::Address(val),
            "num_literal" => Literal::Num(val),
            "bool_literal" => Literal::Bool(val),
            "bytestring_literal" => Literal::ByteString(val),
            _ => unreachable!(),
        }
    }
}

// define_enum!(Literal, {});

// define_ast_node!(AddressLiteral, []);
