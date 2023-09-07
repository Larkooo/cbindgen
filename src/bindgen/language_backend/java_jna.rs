use crate::bindgen::ir::{
    Constant, Documentation, Enum, Field, Function, IntKind, Item, Literal, OpaqueItem,
    PrimitiveType, Static, Struct, Type, Typedef, Union,
};
use crate::bindgen::language_backend::{LanguageBackend, NamespaceOperation};
use crate::bindgen::writer::ListType::Join;
use crate::bindgen::writer::SourceWriter;
use crate::bindgen::{Config, Layout};
use std::fmt::Debug;
use std::io::Write;

pub struct JavaJnaLanguageBackend<'a> {
    config: &'a Config,
    binding_lib_crate_name: String,
}

impl<'a> JavaJnaLanguageBackend<'a> {
    pub fn new(config: &'a Config, binding_lib_crate_name: String) -> Self {
        Self {
            config,
            binding_lib_crate_name,
        }
    }
}

impl LanguageBackend for JavaJnaLanguageBackend<'_> {
    fn write_headers<W: Write>(&self, out: &mut SourceWriter<W>) {
        if let Some(ref header) = self.config.header {
            out.new_line_if_not_start();
            write!(out, "{header}");
            out.new_line();
        }

        if self.config.include_version {
            out.new_line_if_not_start();
            write!(
                out,
                "/* Generated with cbindgen:{} */",
                crate::bindgen::config::VERSION
            );
            out.new_line();
        }
        if let Some(ref autogen_warning) = self.config.autogen_warning {
            out.new_line_if_not_start();
            write!(out, "{autogen_warning}");
            out.new_line();
        }

        if let Some(ref package) = self.config.java_jna.package {
            out.new_line_if_not_start();
            write!(out, "package {package};");
            out.new_line();
            out.new_line();
        }

        out.write("import com.sun.jna.*;");
        out.new_line();
        out.write("import com.sun.jna.ptr.*;");
        out.new_line();
    }

    fn open_close_namespaces<W: Write>(&self, op: NamespaceOperation, out: &mut SourceWriter<W>) {
        if NamespaceOperation::Open == op {
            out.new_line_if_not_start();
            let name = &self
                .config
                .java_jna
                .interface_name
                .clone()
                .unwrap_or("Bindings".to_string());

            write!(out, "enum {}Singleton", name);
            out.open_brace();
            out.write("INSTANCE;");
            out.new_line();

            write!(
                out,
                "final {} lib = Native.load(\"{}\", {}.class);",
                name, self.binding_lib_crate_name, name
            );
            out.close_brace(false);
            out.new_line();
            out.new_line();

            write!(out, "interface {} extends Library", name);
            out.open_brace();

            write!(out, "{} INSTANCE = {}Singleton.INSTANCE.lib;", name, name);
            out.new_line();

            if let Some(extra) = &self.config.java_jna.extra_defs {
                write!(out, "{extra}");
                out.new_line();
            }
        } else {
            out.close_brace(false);
        }
    }

    fn write_footers<W: Write>(&self, _: &mut SourceWriter<W>) {}

    fn write_enum<W: Write>(&self, out: &mut SourceWriter<W>, e: &Enum) {
        self.write_integer_type(
            out,
            &e.documentation,
            &e.export_name,
            JnaIntegerType::Int, /* enum are most of the time the same size as ints */
            &e.annotations.deprecated,
            |out| {
                let mut current_discriminant = 0;
                for variant in &e.variants {
                    current_discriminant = variant
                        .discriminant
                        .clone()
                        .and_then(|it| match it {
                            Literal::Expr(e) => e.parse::<i32>().ok(),
                            _ => None,
                        })
                        .unwrap_or(current_discriminant + 1);
                    self.write_documentation(out, &variant.documentation);
                    write!(
                        out,
                        "public static final {} {} = new {}({});",
                        e.export_name, variant.export_name, e.export_name, current_discriminant
                    );
                    out.new_line();
                }
            },
        );
    }

    fn write_struct<W: Write>(&self, out: &mut SourceWriter<W>, s: &Struct) {
        let constants: Vec<(&Constant, &Struct)> =
            s.associated_constants.iter().map(|it| (it, s)).collect();
        if s.is_transparent {
            let field = s.fields.first();
            match field {
                Some(Field {
                    ty: Type::Primitive(PrimitiveType::Integer { kind, .. }),
                    ..
                }) => {
                    self.write_integer_type(
                        out,
                        &s.documentation,
                        &s.export_name,
                        JnaIntegerType::from_kind(kind),
                        &s.annotations.deprecated,
                        |out| {
                            for (constant, assoc_struct) in constants {
                                constant.write(self.config, self, out, Some(assoc_struct));
                            }
                        },
                    );
                }
                Some(Field {
                    ty: Type::Path(path),
                    ..
                }) => {
                    self.write_jna_struct(
                        out,
                        &JnaStruct {
                            documentation: &s.documentation,
                            constants: &constants,
                            fields: &vec![],
                            name: &s.export_name,
                            superclass: path.export_name(),
                            interface: "Structure.ByValue",
                            deprecated: &s.annotations.deprecated,
                        },
                    );
                    self.write_jna_struct(
                        out,
                        &JnaStruct {
                            documentation: &s.documentation,
                            constants: &constants,
                            fields: &vec![],
                            name: &format!("{}ByReference", s.export_name()),
                            superclass: path.export_name(),
                            interface: "Structure.ByReference",
                            deprecated: &s.annotations.deprecated,
                        },
                    );
                }
                Some(Field {
                    ty: Type::Array(_, _),
                    ..
                }) => self.write_pointer_type(
                    out,
                    &s.documentation,
                    &s.annotations.deprecated,
                    s.export_name(),
                ),
                _ => not_implemented(s, out),
            }
        } else {
            self.write_jna_struct(
                out,
                &JnaStruct {
                    documentation: &s.documentation,
                    constants: &constants,
                    fields: &s.fields,
                    name: &s.export_name,
                    superclass: "Structure",
                    interface: "Structure.ByValue",
                    deprecated: &s.annotations.deprecated,
                },
            );
            self.write_jna_struct(
                out,
                &JnaStruct {
                    documentation: &s.documentation,
                    constants: &constants,
                    fields: &s.fields,
                    name: &format!("{}ByReference", s.export_name),
                    superclass: "Structure",
                    interface: "Structure.ByReference",
                    deprecated: &s.annotations.deprecated,
                },
            );
        }
    }

    fn write_union<W: Write>(&self, out: &mut SourceWriter<W>, u: &Union) {
        self.write_jna_struct(
            out,
            &JnaStruct {
                documentation: &u.documentation,
                constants: &vec![],
                fields: &u.fields,
                name: &u.export_name,
                superclass: "Union",
                interface: "Structure.ByValue",
                deprecated: &u.annotations.deprecated,
            },
        );
        self.write_jna_struct(
            out,
            &JnaStruct {
                documentation: &u.documentation,
                constants: &vec![],
                fields: &u.fields,
                name: &format!("{}ByReference", &u.export_name),
                superclass: "Union",
                interface: "Structure.ByReference",
                deprecated: &u.annotations.deprecated,
            },
        );
    }

    fn write_opaque_item<W: Write>(&self, out: &mut SourceWriter<W>, o: &OpaqueItem) {
        self.write_pointer_type(
            out,
            &o.documentation,
            &o.annotations.deprecated,
            &o.export_name,
        );
    }
    fn write_type_def<W: Write>(&self, out: &mut SourceWriter<W>, t: &Typedef) {
        match &t.aliased {
            Type::FuncPtr { ret, args, .. } => {
                write!(out, "interface {} extends Callback", t.export_name);
                out.open_brace();
                self.write_type(out, ret);
                out.write(" invoke(");
                self.write_indexed_function_args(
                    out,
                    &args
                        .iter()
                        .enumerate()
                        .map(|(index, (name, ty))| IndexedFunctionArg { name, index, ty })
                        .collect::<Vec<_>>(),
                );
                out.write(");");
                out.close_brace(false)
            }
            Type::Path(path) => {
                self.write_documentation(out, &t.documentation);
                write!(
                    out,
                    "class {} extends {}",
                    t.export_name,
                    path.export_name()
                );
                out.open_brace();
                write!(out, "public {}()", t.export_name);
                out.open_brace();
                out.write("super();");
                out.close_brace(false);
                out.new_line();
                write!(out, "public {}(Pointer p)", t.export_name);
                out.open_brace();
                out.write("super(p);");
                out.close_brace(false);
                out.close_brace(false);
                out.new_line();
                out.new_line();
                self.write_documentation(out, &t.documentation);
                write!(
                    out,
                    "class {}ByReference extends {}ByReference",
                    t.export_name,
                    path.export_name()
                );
                out.open_brace();
                write!(out, "public {}ByReference()", t.export_name);
                out.open_brace();
                out.write("super();");
                out.close_brace(false);
                out.new_line();
                write!(out, "public {}ByReference(Pointer p)", t.export_name);
                out.open_brace();
                out.write("super(p);");
                out.close_brace(false);
                out.close_brace(false);
            }
            Type::Primitive(primitive) => match primitive {
                PrimitiveType::Integer { kind, .. } => {
                    let jna_type = JnaIntegerType::from_kind(kind);
                    self.write_integer_type(
                        out,
                        &t.documentation,
                        &t.export_name,
                        jna_type,
                        &t.annotations.deprecated,
                        |_| {},
                    )
                }
                _ => not_implemented(&t, out),
            },
            Type::Ptr { .. } => self.write_pointer_type(
                out,
                &t.documentation,
                &t.annotations.deprecated,
                &t.export_name,
            ),
            Type::Array(_, _) => self.write_pointer_type(
                out,
                &t.documentation,
                &t.annotations.deprecated,
                &t.export_name,
            ),
        }
    }

    fn write_static<W: Write>(&self, out: &mut SourceWriter<W>, s: &Static) {
        not_implemented(s, out)
    }

    fn write_function<W: Write>(&self, out: &mut SourceWriter<W>, f: &Function) {
        self.write_documentation(out, &f.documentation);
        self.write_deprecated(out, &f.annotations.deprecated);
        self.write_type(out, &f.ret);
        write!(out, " {}(", f.path.name());

        self.write_indexed_function_args(
            out,
            &f.args
                .iter()
                .enumerate()
                .map(|(index, arg)| IndexedFunctionArg {
                    name: &arg.name,
                    ty: &arg.ty,
                    index,
                })
                .collect::<Vec<_>>(),
        );

        out.write(");");
    }

    fn write_type<W: Write>(&self, out: &mut SourceWriter<W>, t: &Type) {
        match t {
            Type::Ptr { ty, .. } => match &**ty {
                Type::Ptr { .. } => out.write("PointerByReference"),
                Type::Path(path) => {
                    write!(out, "{}ByReference", path.export_name())
                }
                Type::Primitive(primitive) => {
                    let typ = match primitive {
                        PrimitiveType::Void => "Pointer",
                        PrimitiveType::Bool => "Pointer",
                        PrimitiveType::Char => "ByteByReference",
                        PrimitiveType::SChar => "ByteByReference",
                        PrimitiveType::UChar => "ByteByReference",
                        PrimitiveType::Char32 => "Pointer",
                        PrimitiveType::Float => "FloatByReference",
                        PrimitiveType::Double => "DoubleByReference",
                        PrimitiveType::VaList => "PointerByReference",
                        PrimitiveType::PtrDiffT => "PointerByReference",
                        PrimitiveType::Integer { kind, .. } => {
                            match kind {
                                IntKind::Short => "ShortByReference",
                                IntKind::Int => "IntByReference",
                                IntKind::Long => "NativeLongByReference",
                                IntKind::LongLong => "LongByReference",
                                IntKind::SizeT => "NativeLongByReference", // TODO probably not right
                                IntKind::Size => "NativeLongByReference", // TODO probably not right
                                IntKind::B8 => "ByteByReference",
                                IntKind::B16 => "ShortByReference",
                                IntKind::B32 => "IntByReference",
                                IntKind::B64 => "LongByReference",
                            }
                        }
                    };
                    write!(out, "{typ}")
                }
                Type::Array(_, _) => out.write("Pointer"),
                Type::FuncPtr { .. } => out.write("CallbackReference"),
            },

            Type::Path(path) => {
                write!(out, "{}", path.export_name())
            }
            Type::Primitive(primitive) => {
                //https://github.com/java-native-access/jna/blob/master/www/Mappings.md
                let typ = match primitive {
                    PrimitiveType::Void => "void",
                    PrimitiveType::Bool => "boolean",
                    PrimitiveType::Char => "byte",
                    PrimitiveType::SChar => "byte",
                    PrimitiveType::UChar => "byte",
                    PrimitiveType::Char32 => "char",
                    PrimitiveType::Float => "float",
                    PrimitiveType::Double => "double",
                    PrimitiveType::VaList => "Pointer",
                    PrimitiveType::PtrDiffT => "Pointer",
                    PrimitiveType::Integer { kind, .. } => {
                        match kind {
                            IntKind::Short => "short",
                            IntKind::Int => "int",
                            IntKind::Long => "NativeLong",
                            IntKind::LongLong => "long",
                            IntKind::SizeT => "NativeLong", // TODO probably not right
                            IntKind::Size => "NativeLong",  // TODO probably not right
                            IntKind::B8 => "byte",
                            IntKind::B16 => "short",
                            IntKind::B32 => "int",
                            IntKind::B64 => "long",
                        }
                    }
                };
                write!(out, "{typ}")
            }
            Type::Array(ty, _len) => {
                self.write_type(out, ty);
                out.write("[]");
            }
            Type::FuncPtr { .. } => out.write("Callback"),
        }
    }

    fn write_documentation<W: Write>(&self, out: &mut SourceWriter<W>, d: &Documentation) {
        if !d.doc_comment.is_empty() {
            out.new_line_if_not_start();
            out.write("/**");
            for line in &d.doc_comment {
                out.new_line();
                write!(out, " *{line}")
            }
            out.new_line();
            out.write(" */");
            out.new_line();
        }
    }

    fn write_literal<W: Write>(&self, out: &mut SourceWriter<W>, l: &Literal) {
        match l {
            Literal::Expr(expr) => {
                write!(out, "{expr}")
            }
            Literal::Struct { export_name, .. } => {
                // There is an hashmap in there that doesn't have stable debug output
                not_implemented(&format!("Struct Literal {export_name}"), out)
            }
            _ => not_implemented(l, out),
        }
    }
}

enum JnaIntegerType {
    Byte,
    Short,
    Int,
    NativeLong,
    Long,
    SizeT,
}

impl JnaIntegerType {
    pub fn size(&self) -> &str {
        match self {
            JnaIntegerType::Byte => "1",
            JnaIntegerType::Short => "2",
            JnaIntegerType::Int => "4",
            JnaIntegerType::NativeLong => "Native.LONG_SIZE",
            JnaIntegerType::Long => "8",
            JnaIntegerType::SizeT => "Native.SIZE_T_SIZE",
        }
    }

    pub fn set_method(&self) -> &str {
        match self {
            JnaIntegerType::Byte => "setByte(0, (byte)value.intValue())",
            JnaIntegerType::Short => "setShort(0, (short)value.intValue())",
            JnaIntegerType::Int => "setInt(0, value.intValue())",
            JnaIntegerType::NativeLong | JnaIntegerType::SizeT => {
                "setNativeLong(0, new NativeLong(value.longValue()))"
            }
            JnaIntegerType::Long => "setLong(0, value.longValue())",
        }
    }

    pub fn get_method(&self) -> &str {
        match self {
            JnaIntegerType::Byte => "getByte(0)",
            JnaIntegerType::Short => "getShort(0)",
            JnaIntegerType::Int => "getInt(0)",
            JnaIntegerType::NativeLong | JnaIntegerType::SizeT => "getNativeLong(0).longValue()",
            JnaIntegerType::Long => "getLong(0)",
        }
    }

    pub fn from_kind(kind: &IntKind) -> Self {
        match kind {
            IntKind::Short => JnaIntegerType::Short,
            IntKind::Int => JnaIntegerType::Int,
            IntKind::Long => JnaIntegerType::NativeLong,
            IntKind::LongLong => JnaIntegerType::Long,
            IntKind::SizeT => JnaIntegerType::SizeT,
            IntKind::Size => JnaIntegerType::SizeT,
            IntKind::B8 => JnaIntegerType::Byte,
            IntKind::B16 => JnaIntegerType::Short,
            IntKind::B32 => JnaIntegerType::Int,
            IntKind::B64 => JnaIntegerType::Long,
        }
    }
}

struct JnaStruct<'a> {
    documentation: &'a Documentation,
    constants: &'a Vec<(&'a Constant, &'a Struct)>,
    fields: &'a Vec<Field>,
    name: &'a str,
    superclass: &'a str,
    interface: &'a str,
    deprecated: &'a Option<String>,
}

struct IndexedFunctionArg<'a> {
    ty: &'a Type,
    name: &'a Option<String>,
    index: usize,
}

impl JavaJnaLanguageBackend<'_> {
    fn write_deprecated<F: Write>(&self, out: &mut SourceWriter<F>, deprecated: &Option<String>) {
        if let Some(deprecated) = deprecated {
            if !deprecated.is_empty() {
                out.write("/**");
                out.new_line();
                write!(out, " * @deprecated {}", deprecated);
                out.new_line();
                out.write(" */");
                out.new_line();
            }
            out.write("@Deprecated");
            out.new_line()
        }
    }

    fn write_jna_struct<F: Write>(&self, out: &mut SourceWriter<F>, s: &JnaStruct) {
        out.new_line();
        self.write_documentation(out, s.documentation);
        self.write_deprecated(out, s.deprecated);
        let field_names = s
            .fields
            .iter()
            .map(|it| format!("\"{}\"", it.name))
            .collect::<Vec<_>>();

        if !field_names.is_empty() {
            out.write("@Structure.FieldOrder({");
            if !out.try_write(
                |out| {
                    out.write_horizontal_source_list(self, &field_names, Join(", "), |_, out, s| {
                        write!(out, "{}", s)
                    })
                },
                self.config.line_length,
            ) {
                out.write_vertical_source_list(self, &field_names, Join(","), |_, out, s| {
                    write!(out, "{}", s)
                })
            }
            out.write("})");
            out.new_line();
        }
        write!(
            out,
            "class {} extends {} implements {}",
            s.name, s.superclass, s.interface
        );
        out.open_brace();

        for (constant, assoc_struct) in s.constants {
            constant.write(self.config, self, out, Some(assoc_struct));
        }

        write!(out, "public {}()", s.name);
        out.open_brace();
        out.write("super();");
        out.close_brace(false);
        out.new_line();
        out.new_line();

        write!(out, "public {}(Pointer p)", s.name);
        out.open_brace();
        out.write("super(p);");
        out.close_brace(false);
        out.new_line();
        out.new_line();

        for field in s.fields {
            self.write_documentation(out, &field.documentation);
            out.write("public ");
            self.write_type(out, &field.ty);
            write!(out, " {};", field.name);

            out.new_line()
        }

        out.close_brace(false);
        out.new_line();
    }

    fn write_indexed_function_arg<W: Write>(
        &self,
        out: &mut SourceWriter<W>,
        a: &IndexedFunctionArg,
    ) {
        self.write_type(out, a.ty);
        write!(
            out,
            " {}",
            a.name
                .clone()
                .and_then(|it| if it == "_" { None } else { Some(it) })
                .unwrap_or(format!("arg{}", a.index))
        );
    }

    fn write_indexed_function_args<W: Write>(
        &self,
        out: &mut SourceWriter<W>,
        a: &[IndexedFunctionArg],
    ) {
        match self.config.function.args {
            Layout::Horizontal => out.write_horizontal_source_list(
                self,
                a,
                Join(", "),
                Self::write_indexed_function_arg,
            ),
            Layout::Vertical => out.write_vertical_source_list(
                self,
                a,
                Join(", "),
                Self::write_indexed_function_arg,
            ),
            Layout::Auto => {
                if !out.try_write(
                    |out| {
                        out.write_horizontal_source_list(
                            self,
                            a,
                            Join(", "),
                            Self::write_indexed_function_arg,
                        )
                    },
                    self.config.line_length,
                ) {
                    out.write_vertical_source_list(
                        self,
                        a,
                        Join(", "),
                        Self::write_indexed_function_arg,
                    )
                }
            }
        }
    }

    fn write_integer_type<W: Write, F: FnOnce(&mut SourceWriter<W>)>(
        &self,
        out: &mut SourceWriter<W>,
        documentation: &Documentation,
        name: &str,
        jna_underlying_type: JnaIntegerType,
        deprecated: &Option<String>,
        extra: F,
    ) {
        let size = jna_underlying_type.size();
        self.write_documentation(out, documentation);
        self.write_deprecated(out, deprecated);
        write!(out, "class {} extends IntegerType", name);
        out.open_brace();
        write!(out, "public {}()", name);
        out.open_brace();
        write!(out, "super({size});");
        out.close_brace(false);
        out.new_line();
        out.new_line();
        write!(out, "public {}(long value)", name);
        out.open_brace();
        write!(out, "super({size}, value);");
        out.close_brace(false);
        out.new_line();
        out.new_line();
        write!(out, "public {}(Pointer p)", name);
        out.open_brace();
        write!(out, "this(p.{});", jna_underlying_type.get_method(),);
        out.close_brace(false);
        out.new_line();
        extra(out);
        out.close_brace(false);
        out.new_line();
        out.new_line();

        write!(out, "class {}ByReference extends ByReference", name);
        out.open_brace();
        write!(out, "public {}ByReference()", name);
        out.open_brace();
        write!(out, "super({size});");
        out.close_brace(false);
        out.new_line();
        out.new_line();
        write!(out, "public {}ByReference(Pointer p)", name);
        out.open_brace();
        write!(out, "super({size});");
        out.new_line();
        out.write("setPointer(p);");
        out.close_brace(false);
        out.new_line();
        out.new_line();
        write!(out, "public {name} getValue()");
        out.open_brace();
        write!(
            out,
            "return new {}(getPointer().{});",
            name,
            jna_underlying_type.get_method()
        );
        out.close_brace(false);
        out.new_line();
        out.new_line();
        write!(out, "public void setValue({name} value)");
        out.open_brace();
        write!(out, "getPointer().{};", jna_underlying_type.set_method());
        out.close_brace(false);
        out.new_line();
        out.close_brace(false);
    }

    fn write_pointer_type<W: Write>(
        &self,
        out: &mut SourceWriter<W>,
        documentation: &Documentation,
        deprecated: &Option<String>,
        name: &str,
    ) {
        self.write_documentation(out, documentation);
        self.write_deprecated(out, deprecated);
        write!(out, "class {} extends PointerType", name);
        out.open_brace();
        write!(out, "public {}()", name);
        out.open_brace();
        out.write("super(null);");
        out.close_brace(false);
        out.new_line();
        write!(out, "public {}(Pointer p)", name);
        out.open_brace();
        out.write("super(p);");
        out.close_brace(false);
        out.close_brace(false);
        out.new_line();
        out.new_line();
        self.write_documentation(out, documentation);
        self.write_deprecated(out, deprecated);
        write!(out, "class {}ByReference extends {}", name, name,);
        out.open_brace();
        write!(out, "public {}ByReference()", name);
        out.open_brace();
        out.write("super(null);");
        out.close_brace(false);
        out.new_line();
        write!(out, "public {}ByReference(Pointer p)", name);
        out.open_brace();
        out.write("super(p);");
        out.close_brace(false);
        out.close_brace(false);
    }
}

pub(crate) fn wrap_java_value(literal: &Literal, ty: &Type) -> Literal {
    match literal {
        Literal::Expr(expr) => match ty {
            Type::Primitive(primitive) => match primitive {
                PrimitiveType::Double => Literal::Expr(format!("{expr}d")),
                PrimitiveType::Float => Literal::Expr(format!("{expr}f")),
                PrimitiveType::Integer {
                    kind: IntKind::LongLong | IntKind::B64,
                    ..
                } => Literal::Expr(format!("{expr}L")),
                PrimitiveType::Integer {
                    kind: IntKind::Long | IntKind::Size | IntKind::SizeT,
                    ..
                } => Literal::Expr(format!("new NativeLong({expr})")),

                _ => literal.clone(),
            },
            Type::Path(path) => Literal::Expr(format!("new {}({expr})", path.export_name())),
            _ => literal.clone(),
        },
        _ => literal.clone(),
    }
}

pub(crate) fn java_writable_literal(ty: &Type, literal: &Literal) -> bool {
    // quite crude for now
    match literal {
        Literal::Expr(e) => {
            !((ty == &Type::Primitive(PrimitiveType::Char32) && e.starts_with("U'\\U"))
                || (matches!(ty, Type::Primitive(PrimitiveType::Integer { .. }))
                    || e.ends_with("ull")))
        }
        _ => false,
    }
}

fn not_implemented<T: Debug, F: Write>(value: &T, out: &mut SourceWriter<F>) {
    write!(out, "/* Not implemented yet : {value:?} */")
}
