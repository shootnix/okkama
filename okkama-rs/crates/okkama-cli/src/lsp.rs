//! `okkama lsp` — сервер языка (Language Server Protocol, stdio).
//!
//! Каждый открытый файл после изменения проверяется заново целиком: разбор,
//! импорты (открытые файлы берутся из буферов редактора), мягкий анализ
//! ([`okkama_compiler::lowering::inspect`]). Проверка программы со всей std
//! занимает ~0.1 с, поэтому кэша нет; изменения, пришедшие подряд,
//! объединяются в одну проверку.
//!
//! Возможности: диагностики, наведение, переход к определению, структура
//! файла. Пока файл не разбирается, наведение и переход используют
//! последний разобранный вариант.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;

use lsp_server::{Connection, Message, Notification, Request, Response};
use lsp_types::{
    DiagnosticSeverity, DocumentSymbol, DocumentSymbolResponse, GotoDefinitionResponse, Hover,
    HoverContents, HoverProviderCapability, MarkupContent, MarkupKind, OneOf,
    PublishDiagnosticsParams, ServerCapabilities, ServerInfo, SymbolKind,
    TextDocumentSyncCapability, TextDocumentSyncKind, Uri,
    notification::{
        DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, DidSaveTextDocument,
        Notification as _, PublishDiagnostics,
    },
    request::{
        Completion, DocumentHighlightRequest, DocumentSymbolRequest, Formatting, GotoDefinition,
        HoverRequest, PrepareRenameRequest, References, Rename, Request as _, SignatureHelpRequest,
    },
};
use okkama_compiler::ide::{self, Inspection, LineIndex, OutlineItem, OutlineKind};
use okkama_compiler::{SourceFile, lowering};
use serde_json::Value;

use super::FileLoader;

/// Открытый файл.
struct Document {
    path: PathBuf,
    text: String,
    inspection: Inspection,
    /// Последний разобранный текст и его результат (наведение при ошибке разбора).
    good: Option<(String, Inspection)>,
    /// Пути, в которые публиковались диагностики этого файла.
    published: Vec<String>,
}

struct Server {
    connection: Connection,
    documents: HashMap<String, Document>,
    stdlib: Option<PathBuf>,
    import_dirs: Vec<PathBuf>,
}

pub fn run() -> ExitCode {
    let (connection, threads) = Connection::stdio();
    let capabilities = ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        definition_provider: Some(OneOf::Left(true)),
        document_symbol_provider: Some(OneOf::Left(true)),
        document_formatting_provider: Some(OneOf::Left(true)),
        references_provider: Some(OneOf::Left(true)),
        document_highlight_provider: Some(OneOf::Left(true)),
        rename_provider: Some(OneOf::Right(lsp_types::RenameOptions {
            prepare_provider: Some(true),
            work_done_progress_options: lsp_types::WorkDoneProgressOptions::default(),
        })),
        completion_provider: Some(lsp_types::CompletionOptions {
            trigger_characters: Some(vec![".".to_string()]),
            ..lsp_types::CompletionOptions::default()
        }),
        signature_help_provider: Some(lsp_types::SignatureHelpOptions {
            trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
            ..lsp_types::SignatureHelpOptions::default()
        }),
        ..ServerCapabilities::default()
    };
    let result = serde_json::json!({
        "capabilities": capabilities,
        "serverInfo": ServerInfo {
            name: "okkama".to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
        },
    });
    let params = match connection.initialize_start() {
        Ok((id, params)) => {
            if connection.initialize_finish(id, result).is_err() {
                return ExitCode::FAILURE;
            }
            params
        }
        Err(error) => {
            eprintln!("okkama lsp: {error}");
            return ExitCode::FAILURE;
        }
    };
    let options = params
        .get("initializationOptions")
        .cloned()
        .unwrap_or(Value::Null);
    let stdlib = options
        .get("stdlibPath")
        .and_then(Value::as_str)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from);
    let import_dirs = options
        .get("importDirs")
        .and_then(Value::as_array)
        .map(|dirs| {
            dirs.iter()
                .filter_map(Value::as_str)
                .map(PathBuf::from)
                .collect()
        })
        .unwrap_or_default();
    let mut server = Server {
        connection,
        documents: HashMap::new(),
        stdlib,
        import_dirs,
    };
    let code = server.serve();
    drop(server);
    let _ = threads.join();
    code
}

impl Server {
    fn serve(&mut self) -> ExitCode {
        loop {
            let Ok(message) = self.connection.receiver.recv() else {
                return ExitCode::SUCCESS;
            };
            let mut dirty = Vec::new();
            if self.handle(message, &mut dirty) {
                return ExitCode::SUCCESS;
            }
            // Изменения подряд — одна проверка.
            while let Ok(message) = self.connection.receiver.try_recv() {
                if self.handle(message, &mut dirty) {
                    return ExitCode::SUCCESS;
                }
            }
            dirty.sort();
            dirty.dedup();
            for uri in dirty {
                self.check(&uri);
            }
        }
    }

    /// `true` — конец сеанса.
    fn handle(&mut self, message: Message, dirty: &mut Vec<String>) -> bool {
        match message {
            Message::Request(request) => {
                if self.connection.handle_shutdown(&request).unwrap_or(true) {
                    return true;
                }
                // Запрос отвечает по текущему тексту: сначала незавершённые проверки.
                for uri in dirty.drain(..) {
                    self.check(&uri);
                }
                let response = self.request(request);
                let _ = self.connection.sender.send(Message::Response(response));
            }
            Message::Notification(notification) => self.notification(notification, dirty),
            Message::Response(_) => {}
        }
        false
    }

    fn notification(&mut self, notification: Notification, dirty: &mut Vec<String>) {
        let params = notification.params;
        match notification.method.as_str() {
            DidOpenTextDocument::METHOD => {
                let Some(document) = params.get("textDocument") else {
                    return;
                };
                let (Some(uri), Some(text)) = (
                    document.get("uri").and_then(Value::as_str),
                    document.get("text").and_then(Value::as_str),
                ) else {
                    return;
                };
                let Some(path) = uri_path(uri) else {
                    return;
                };
                self.documents.insert(
                    uri.to_string(),
                    Document {
                        path,
                        text: text.to_string(),
                        inspection: Inspection::default(),
                        good: None,
                        published: Vec::new(),
                    },
                );
                // Сразу: правки, пришедшие следом, не должны лишить файл
                // разобранного варианта (дополнение опирается на него).
                self.check(uri);
            }
            DidChangeTextDocument::METHOD => {
                let Some(uri) = params.pointer("/textDocument/uri").and_then(Value::as_str) else {
                    return;
                };
                let text = params
                    .get("contentChanges")
                    .and_then(Value::as_array)
                    .and_then(|changes| changes.last())
                    .and_then(|change| change.get("text"))
                    .and_then(Value::as_str);
                if let (Some(document), Some(text)) = (self.documents.get_mut(uri), text) {
                    document.text = text.to_string();
                    dirty.push(uri.to_string());
                }
                // Изменение файла меняет и проверку импортирующих его.
                dirty.extend(self.documents.keys().filter(|other| *other != uri).cloned());
            }
            DidSaveTextDocument::METHOD => {
                if let Some(uri) = params.pointer("/textDocument/uri").and_then(Value::as_str) {
                    dirty.push(uri.to_string());
                }
            }
            DidCloseTextDocument::METHOD => {
                let Some(uri) = params.pointer("/textDocument/uri").and_then(Value::as_str) else {
                    return;
                };
                if let Some(document) = self.documents.remove(uri) {
                    for path in document.published {
                        self.publish(&path, Vec::new());
                    }
                }
                dirty.retain(|other| other != uri);
            }
            _ => {}
        }
    }

    fn loader(&self, test_mode: bool) -> FileLoader {
        let mut loader = FileLoader::from_env();
        if let Some(stdlib) = &self.stdlib {
            loader.stdlib = stdlib.clone();
        }
        loader.import_dirs.extend(self.import_dirs.iter().cloned());
        loader.test_mode = test_mode;
        for document in self.documents.values() {
            let canonical =
                std::fs::canonicalize(&document.path).unwrap_or_else(|_| document.path.clone());
            loader.overlay.insert(canonical, document.text.clone());
        }
        loader
    }

    fn check(&mut self, uri: &str) {
        let Some(document) = self.documents.get(uri) else {
            return;
        };
        let name = std::fs::canonicalize(&document.path)
            .unwrap_or_else(|_| document.path.clone())
            .display()
            .to_string();
        let Ok(source) = SourceFile::new(name, document.text.clone()) else {
            return;
        };
        let test_mode = ide::has_tests(&source);
        let mut loader = self.loader(test_mode);
        let inspection = lowering::inspect(&source, &mut loader, test_mode);

        // Диагностики по файлам; ошибки других файлов — ещё и на первом импорте корня.
        let mut by_path: HashMap<String, Vec<lsp_types::Diagnostic>> = HashMap::new();
        by_path.entry(source.name().to_string()).or_default();
        let import_range = inspection
            .diagnostics
            .iter()
            .any(|found| found.path != source.name())
            .then(|| first_import_range(&document.text));
        for found in &inspection.diagnostics {
            let mut diagnostic = lsp_diagnostic(found.range, found.severity, found.message.clone());
            if let Some(code) = &found.code {
                diagnostic.code = Some(lsp_types::NumberOrString::String(code.clone()));
            }
            if found.unnecessary {
                diagnostic.tags = Some(vec![lsp_types::DiagnosticTag::UNNECESSARY]);
            }
            if found.path != source.name() {
                if let Some(range) = import_range {
                    let file = Path::new(&found.path)
                        .file_name()
                        .map_or(found.path.clone(), |name| {
                            name.to_string_lossy().into_owned()
                        });
                    by_path
                        .entry(source.name().to_string())
                        .or_default()
                        .push(lsp_diagnostic(
                            range,
                            found.severity,
                            format!("{file}:{}: {}", found.range.start.line + 1, found.message),
                        ));
                }
            }
            by_path
                .entry(found.path.clone())
                .or_default()
                .push(diagnostic);
        }
        let previous = self
            .documents
            .get(uri)
            .map(|document| document.published.clone())
            .unwrap_or_default();
        for path in previous.iter().filter(|path| !by_path.contains_key(*path)) {
            self.publish(path, Vec::new());
        }
        let published: Vec<String> = by_path.keys().cloned().collect();
        for (path, diagnostics) in by_path {
            self.publish(&path, diagnostics);
        }
        if let Some(document) = self.documents.get_mut(uri) {
            if inspection.parsed {
                document.good = Some((document.text.clone(), inspection.clone()));
            }
            document.inspection = inspection;
            document.published = published;
        }
    }

    fn publish(&self, path: &str, diagnostics: Vec<lsp_types::Diagnostic>) {
        let Some(uri) = path_uri(path) else {
            return;
        };
        let params = PublishDiagnosticsParams {
            uri,
            diagnostics,
            version: None,
        };
        let _ = self
            .connection
            .sender
            .send(Message::Notification(Notification::new(
                PublishDiagnostics::METHOD.to_string(),
                params,
            )));
    }

    fn request(&mut self, request: Request) -> Response {
        let id = request.id.clone();
        let params = &request.params;
        let result = match request.method.as_str() {
            HoverRequest::METHOD => self.hover(params).map_or(Value::Null, |hover| {
                serde_json::to_value(hover).unwrap_or(Value::Null)
            }),
            GotoDefinition::METHOD => self.definition(params).map_or(Value::Null, |found| {
                serde_json::to_value(found).unwrap_or(Value::Null)
            }),
            DocumentSymbolRequest::METHOD => self.outline(params).map_or(Value::Null, |found| {
                serde_json::to_value(found).unwrap_or(Value::Null)
            }),
            Completion::METHOD => self.completion(params).map_or(Value::Null, |found| {
                serde_json::to_value(found).unwrap_or(Value::Null)
            }),
            SignatureHelpRequest::METHOD => {
                self.signature_help(params).map_or(Value::Null, |found| {
                    serde_json::to_value(found).unwrap_or(Value::Null)
                })
            }
            References::METHOD => self.references(params).map_or(Value::Null, |found| {
                serde_json::to_value(found).unwrap_or(Value::Null)
            }),
            DocumentHighlightRequest::METHOD => {
                self.highlights(params).map_or(Value::Null, |found| {
                    serde_json::to_value(found).unwrap_or(Value::Null)
                })
            }
            PrepareRenameRequest::METHOD => match self.renamable(params) {
                Ok(range) => serde_json::to_value(range).unwrap_or(Value::Null),
                Err(message) => {
                    return Response::new_err(
                        id,
                        lsp_server::ErrorCode::RequestFailed as i32,
                        message,
                    );
                }
            },
            Rename::METHOD => match self.rename(params) {
                Ok(edit) => serde_json::to_value(edit).unwrap_or(Value::Null),
                Err(message) => {
                    return Response::new_err(
                        id,
                        lsp_server::ErrorCode::RequestFailed as i32,
                        message,
                    );
                }
            },
            Formatting::METHOD => match self.format(params) {
                Ok(edits) => serde_json::to_value(edits).unwrap_or(Value::Null),
                Err(message) => {
                    return Response::new_err(
                        id,
                        lsp_server::ErrorCode::RequestFailed as i32,
                        message,
                    );
                }
            },
            _ => {
                return Response::new_err(
                    id,
                    lsp_server::ErrorCode::MethodNotFound as i32,
                    format!("unsupported request {}", request.method),
                );
            }
        };
        Response::new_ok(id, result)
    }

    /// Символ под позицией запроса: из текущей проверки или, если текст не
    /// разобран, из последней разобранной.
    fn symbol(&self, params: &Value) -> Option<ide::Symbol> {
        let uri = params.pointer("/textDocument/uri")?.as_str()?;
        let line = params.pointer("/position/line")?.as_u64()? as u32;
        let character = params.pointer("/position/character")?.as_u64()? as u32;
        let document = self.documents.get(uri)?;
        let position = ide::Position { line, character };
        let (text, inspection) = if document.inspection.parsed {
            (&document.text, &document.inspection)
        } else {
            let (text, inspection) = document.good.as_ref()?;
            (text, inspection)
        };
        let offset = LineIndex::new(text).offset(position);
        inspection.symbol_at(offset).cloned()
    }

    /// Документ, позиция запроса в его текущем тексте и последняя разобранная
    /// проверка (её текст и результат).
    fn position(&self, params: &Value) -> Option<(&Document, u32, &str, &Inspection)> {
        let uri = params.pointer("/textDocument/uri")?.as_str()?;
        let line = params.pointer("/position/line")?.as_u64()? as u32;
        let character = params.pointer("/position/character")?.as_u64()? as u32;
        let document = self.documents.get(uri)?;
        let offset = LineIndex::new(&document.text).offset(ide::Position { line, character });
        let (text, inspection) = if document.inspection.parsed {
            (document.text.as_str(), &document.inspection)
        } else {
            let (text, inspection) = document.good.as_ref()?;
            (text.as_str(), inspection)
        };
        Some((document, offset, text, inspection))
    }

    /// Путь `std.…` у курсора (§5.2.1): папки, модули и объявления std с
    /// диска. Не зависит от анализа документа: `std.` посреди набора ещё не
    /// разбирается.
    fn std_completion(&self, params: &Value) -> Option<lsp_types::CompletionResponse> {
        let uri = params.pointer("/textDocument/uri")?.as_str()?;
        let line = params.pointer("/position/line")?.as_u64()? as u32;
        let character = params.pointer("/position/character")?.as_u64()? as u32;
        let document = self.documents.get(uri)?;
        let offset = LineIndex::new(&document.text).offset(ide::Position { line, character });
        let found = ide::std_completions(&document.text, offset, &self.loader(false).stdlib)?;
        let items = found
            .into_iter()
            .map(|found| lsp_types::CompletionItem {
                kind: Some(match found.kind {
                    ide::StdKind::Folder => lsp_types::CompletionItemKind::FOLDER,
                    ide::StdKind::Module => lsp_types::CompletionItemKind::MODULE,
                    ide::StdKind::Object => lsp_types::CompletionItemKind::CLASS,
                    ide::StdKind::Role => lsp_types::CompletionItemKind::INTERFACE,
                    ide::StdKind::Enum => lsp_types::CompletionItemKind::ENUM,
                    ide::StdKind::EnumCase => lsp_types::CompletionItemKind::ENUM_MEMBER,
                    ide::StdKind::Native => lsp_types::CompletionItemKind::FUNCTION,
                }),
                // Сначала текущий уровень, потом целые пути из глубины.
                sort_text: Some(format!("{}{}", u8::from(found.deep), found.label)),
                detail: (!found.detail.is_empty()).then_some(found.detail),
                documentation: (!found.documentation.is_empty()).then_some(
                    lsp_types::Documentation::MarkupContent(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: found.documentation,
                    }),
                ),
                label: found.label,
                ..lsp_types::CompletionItem::default()
            })
            .collect();
        Some(lsp_types::CompletionResponse::Array(items))
    }

    fn completion(&self, params: &Value) -> Option<lsp_types::CompletionResponse> {
        if let Some(found) = self.std_completion(params) {
            return Some(found);
        }
        let (document, offset, analyzed, inspection) = self.position(params)?;
        let items = inspection
            .complete(analyzed, &document.text, offset)
            .into_iter()
            .map(|found| lsp_types::CompletionItem {
                label: found.label,
                kind: Some(match found.kind {
                    ide::CompletionKind::Method => lsp_types::CompletionItemKind::METHOD,
                    ide::CompletionKind::Field => lsp_types::CompletionItemKind::FIELD,
                    ide::CompletionKind::Variable => lsp_types::CompletionItemKind::VARIABLE,
                    ide::CompletionKind::Object => lsp_types::CompletionItemKind::CLASS,
                    ide::CompletionKind::Role => lsp_types::CompletionItemKind::INTERFACE,
                    ide::CompletionKind::Enum => lsp_types::CompletionItemKind::ENUM,
                    ide::CompletionKind::EnumMember => lsp_types::CompletionItemKind::ENUM_MEMBER,
                    ide::CompletionKind::Function => lsp_types::CompletionItemKind::FUNCTION,
                    ide::CompletionKind::Keyword => lsp_types::CompletionItemKind::KEYWORD,
                    ide::CompletionKind::Module => lsp_types::CompletionItemKind::MODULE,
                }),
                detail: (!found.detail.is_empty()).then_some(found.detail),
                documentation: (!found.documentation.is_empty()).then_some(
                    lsp_types::Documentation::MarkupContent(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: found.documentation,
                    }),
                ),
                insert_text: Some(found.insert),
                insert_text_format: Some(if found.snippet {
                    lsp_types::InsertTextFormat::SNIPPET
                } else {
                    lsp_types::InsertTextFormat::PLAIN_TEXT
                }),
                ..lsp_types::CompletionItem::default()
            })
            .collect();
        Some(lsp_types::CompletionResponse::Array(items))
    }

    fn signature_help(&self, params: &Value) -> Option<lsp_types::SignatureHelp> {
        let (document, offset, analyzed, inspection) = self.position(params)?;
        let (label, parameters, active) = inspection.signature(analyzed, &document.text, offset)?;
        Some(lsp_types::SignatureHelp {
            signatures: vec![lsp_types::SignatureInformation {
                label,
                documentation: None,
                parameters: Some(
                    parameters
                        .into_iter()
                        .map(|parameter| lsp_types::ParameterInformation {
                            label: lsp_types::ParameterLabel::Simple(parameter),
                            documentation: None,
                        })
                        .collect(),
                ),
                active_parameter: Some(active as u32),
            }],
            active_signature: Some(0),
            active_parameter: Some(active as u32),
        })
    }

    /// Вхождения имени под курсором в текущем (разобранном) тексте.
    fn occurrences(&self, params: &Value) -> Option<(String, Vec<ide::Symbol>, LineIndex<'_>)> {
        let uri = params.pointer("/textDocument/uri")?.as_str()?;
        let document = self.documents.get(uri)?;
        if !document.inspection.parsed {
            return None;
        }
        let line = params.pointer("/position/line")?.as_u64()? as u32;
        let character = params.pointer("/position/character")?.as_u64()? as u32;
        let lines = LineIndex::new(&document.text);
        let offset = lines.offset(ide::Position { line, character });
        let found: Vec<ide::Symbol> = document
            .inspection
            .occurrences(offset)
            .into_iter()
            .cloned()
            .collect();
        Some((uri.to_string(), found, lines))
    }

    fn references(&self, params: &Value) -> Option<Vec<lsp_types::Location>> {
        let (uri, found, lines) = self.occurrences(params)?;
        let uri = Uri::from_str(&uri).ok()?;
        let include = params
            .pointer("/context/includeDeclaration")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let mut out: Vec<lsp_types::Location> = found
            .iter()
            .map(|symbol| lsp_types::Location {
                uri: uri.clone(),
                range: lsp_range(lines.range(symbol.span)),
            })
            .collect();
        if let Some(definition) = found.first().and_then(|symbol| symbol.definition.clone()) {
            let location = lsp_types::Location {
                uri: path_uri(&definition.path)?,
                range: lsp_range(definition.range),
            };
            if !include {
                out.retain(|found| found.range != location.range);
            } else if !out.iter().any(|found| found.range == location.range) {
                out.push(location);
            }
        }
        Some(out)
    }

    fn highlights(&self, params: &Value) -> Option<Vec<lsp_types::DocumentHighlight>> {
        let (_, found, lines) = self.occurrences(params)?;
        Some(
            found
                .iter()
                .map(|symbol| lsp_types::DocumentHighlight {
                    range: lsp_range(lines.range(symbol.span)),
                    kind: Some(lsp_types::DocumentHighlightKind::TEXT),
                })
                .collect(),
        )
    }

    /// Переименовываются только локальные и параметры: все их вхождения в этом файле.
    fn renamable(&self, params: &Value) -> Result<lsp_types::Range, String> {
        let (_, found, lines) = self
            .occurrences(params)
            .ok_or("the file must parse before renaming")?;
        match found.first() {
            Some(symbol) if found.iter().all(|symbol| symbol.local) => {
                Ok(lsp_range(lines.range(symbol.span)))
            }
            Some(_) => Err("only local variables and parameters can be renamed".to_string()),
            None => Err("no name here".to_string()),
        }
    }

    // `WorkspaceEdit::changes` — `HashMap<Uri, _>` по определению lsp-types.
    #[allow(clippy::mutable_key_type)]
    fn rename(&self, params: &Value) -> Result<lsp_types::WorkspaceEdit, String> {
        self.renamable(params)?;
        let name = params
            .get("newName")
            .and_then(Value::as_str)
            .ok_or("missing new name")?;
        let valid = name.starts_with(|ch: char| ch.is_ascii_lowercase() || ch == '_')
            && name
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            && !ide::keywords().iter().any(|keyword| keyword.label == name)
            && ![
                "self", "next", "last", "true", "false", "none", "null", "nil",
            ]
            .contains(&name);
        if !valid {
            return Err(format!("`{name}` is not a valid local name"));
        }
        let (uri, found, lines) = self
            .occurrences(params)
            .ok_or("the file must parse before renaming")?;
        let uri = Uri::from_str(&uri).map_err(|error| error.to_string())?;
        let edits = found
            .iter()
            .map(|symbol| lsp_types::TextEdit {
                range: lsp_range(lines.range(symbol.span)),
                new_text: name.to_string(),
            })
            .collect();
        let mut changes = HashMap::new();
        changes.insert(uri, edits);
        Ok(lsp_types::WorkspaceEdit {
            changes: Some(changes),
            ..lsp_types::WorkspaceEdit::default()
        })
    }

    fn hover(&self, params: &Value) -> Option<Hover> {
        let symbol = self.symbol(params)?;
        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: symbol.hover,
            }),
            range: None,
        })
    }

    fn definition(&self, params: &Value) -> Option<GotoDefinitionResponse> {
        let location = self.symbol(params)?.definition?;
        Some(GotoDefinitionResponse::Scalar(lsp_types::Location {
            uri: path_uri(&location.path)?,
            range: lsp_range(location.range),
        }))
    }

    /// Весь документ заменяется отформатированным текстом; файл с
    /// синтаксической ошибкой не форматируется (ошибка уже в диагностиках).
    fn format(&self, params: &Value) -> Result<Vec<lsp_types::TextEdit>, String> {
        let Some(document) = params
            .pointer("/textDocument/uri")
            .and_then(Value::as_str)
            .and_then(|uri| self.documents.get(uri))
        else {
            return Ok(Vec::new());
        };
        let source = SourceFile::new(document.path.display().to_string(), document.text.clone())
            .map_err(|error| error.to_string())?;
        let formatted = match okkama_compiler::syntax::format::format(&source) {
            Ok(formatted) => formatted,
            Err(okkama_compiler::syntax::format::FormatError::Syntax(_)) => return Ok(Vec::new()),
            Err(okkama_compiler::syntax::format::FormatError::Internal(message)) => {
                return Err(format!("okkama fmt: {message}"));
            }
        };
        if formatted == document.text {
            return Ok(Vec::new());
        }
        let lines = LineIndex::new(&document.text);
        let end = lines.position(document.text.len() as u32);
        Ok(vec![lsp_types::TextEdit {
            range: lsp_range(ide::Range {
                start: ide::Position::default(),
                end,
            }),
            new_text: formatted,
        }])
    }

    fn outline(&self, params: &Value) -> Option<DocumentSymbolResponse> {
        let uri = params.pointer("/textDocument/uri")?.as_str()?;
        let document = self.documents.get(uri)?;
        let inspection = if document.inspection.parsed {
            &document.inspection
        } else {
            &document.good.as_ref()?.1
        };
        Some(DocumentSymbolResponse::Nested(
            inspection.outline.iter().map(document_symbol).collect(),
        ))
    }
}

fn first_import_range(text: &str) -> ide::Range {
    let lines = LineIndex::new(text);
    let start = text.find("import").unwrap_or(0) as u32;
    let end = text[start as usize..]
        .find('\n')
        .map_or(text.len() as u32, |at| start + at as u32);
    lines.range(okkama_compiler::Span::new(start, end))
}

fn lsp_position(position: ide::Position) -> lsp_types::Position {
    lsp_types::Position {
        line: position.line,
        character: position.character,
    }
}

fn lsp_range(range: ide::Range) -> lsp_types::Range {
    lsp_types::Range {
        start: lsp_position(range.start),
        end: lsp_position(range.end),
    }
}

fn lsp_diagnostic(
    range: ide::Range,
    severity: ide::Severity,
    message: String,
) -> lsp_types::Diagnostic {
    lsp_types::Diagnostic {
        range: lsp_range(range),
        severity: Some(match severity {
            ide::Severity::Error => DiagnosticSeverity::ERROR,
            ide::Severity::Warning => DiagnosticSeverity::WARNING,
        }),
        source: Some("okkama".to_string()),
        message,
        ..lsp_types::Diagnostic::default()
    }
}

#[allow(deprecated)] // `DocumentSymbol::deprecated` обязателен в 0.97.
fn document_symbol(item: &OutlineItem) -> DocumentSymbol {
    DocumentSymbol {
        name: item.name.clone(),
        detail: (!item.detail.is_empty()).then(|| item.detail.clone()),
        kind: match item.kind {
            OutlineKind::Object => SymbolKind::CLASS,
            OutlineKind::Role => SymbolKind::INTERFACE,
            OutlineKind::Enum => SymbolKind::ENUM,
            OutlineKind::EnumCase => SymbolKind::ENUM_MEMBER,
            OutlineKind::Field => SymbolKind::FIELD,
            OutlineKind::Constructor => SymbolKind::CONSTRUCTOR,
            OutlineKind::Action => SymbolKind::METHOD,
            OutlineKind::Cap => SymbolKind::STRUCT,
            OutlineKind::Native => SymbolKind::FUNCTION,
            OutlineKind::Test => SymbolKind::EVENT,
            OutlineKind::Main => SymbolKind::FUNCTION,
        },
        tags: None,
        deprecated: None,
        range: lsp_range(item.range),
        selection_range: lsp_range(item.selection),
        children: (!item.children.is_empty())
            .then(|| item.children.iter().map(document_symbol).collect()),
    }
}

/// `file:///a/b%20c.ok` → `/a/b c.ok`.
fn uri_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    Some(PathBuf::from(String::from_utf8(out).ok()?))
}

/// Путь → `file://` URI с процентным кодированием.
fn path_uri(path: &str) -> Option<Uri> {
    let mut text = String::from("file://");
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(&byte) {
            text.push(byte as char);
        } else {
            text.push_str(&format!("%{byte:02X}"));
        }
    }
    Uri::from_str(&text).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_uris_round_trip_with_escapes() {
        let path = "/Volumes/My Disk/проект/a+b.ok";
        let uri = path_uri(path).unwrap();
        assert_eq!(
            uri.as_str(),
            "file:///Volumes/My%20Disk/%D0%BF%D1%80%D0%BE%D0%B5%D0%BA%D1%82/a%2Bb.ok"
        );
        assert_eq!(uri_path(uri.as_str()).unwrap(), PathBuf::from(path));
        assert_eq!(
            uri_path("file:///a%3Ab.ok").unwrap(),
            PathBuf::from("/a:b.ok")
        );
    }
}
