// Клиент сервера языка Okkama: запускает `okkama lsp` и подключает его к VS Code.
//
// Сервер — Rust-реализация (`okkama-rs`); Perl-обёртка `okkama` из PATH
// команды `lsp` не знает. Путь берётся из `okkama.serverPath`, иначе ищется
// сборка `okkama-rs/target/release/okkama` в рабочей области, затем места
// установки (`~/.local/bin`, `/usr/local/bin`, `/opt/homebrew/bin`) и PATH —
// первый, кто отвечает на `okkama env`.

const childProcess = require("child_process");
const fs = require("fs");
const os = require("os");
const path = require("path");
const vscode = require("vscode");
const { LanguageClient } = require("vscode-languageclient/node");

let client;

/** Rust-сборка `okkama`: отвечает на `okkama env` (у Perl-версии такой команды нет). */
function isRustOkkama(candidate) {
  try {
    const out = childProcess.execFileSync(candidate, ["env"], { timeout: 3000, encoding: "utf8" });
    return out.startsWith("okkama ");
  } catch (error) {
    return false;
  }
}

function serverPath(config) {
  const configured = config.get("serverPath");
  if (configured) {
    return configured;
  }
  const candidates = [];
  for (const folder of vscode.workspace.workspaceFolders || []) {
    candidates.push(path.join(folder.uri.fsPath, "okkama-rs", "target", "release", "okkama"));
  }
  // Приложение из Dock видит не тот PATH, что терминал: известные места установки.
  candidates.push(
    path.join(os.homedir(), ".local", "bin", "okkama"),
    "/usr/local/bin/okkama",
    "/opt/homebrew/bin/okkama",
    "okkama"
  );
  for (const candidate of candidates) {
    if ((candidate === "okkama" || fs.existsSync(candidate)) && isRustOkkama(candidate)) {
      return candidate;
    }
  }
  return "okkama";
}

function stdlibPath(config) {
  const configured = config.get("stdlibPath");
  if (configured) {
    return configured;
  }
  for (const folder of vscode.workspace.workspaceFolders || []) {
    const candidate = path.join(folder.uri.fsPath, "std");
    if (fs.existsSync(path.join(candidate, "io", "stdout.ok"))) {
      return candidate;
    }
  }
  return "";
}

async function start() {
  const config = vscode.workspace.getConfiguration("okkama");
  const command = serverPath(config);
  const serverOptions = {
    run: { command, args: ["lsp"] },
    debug: { command, args: ["lsp"] },
  };
  const clientOptions = {
    documentSelector: [{ scheme: "file", language: "okkama" }],
    initializationOptions: {
      stdlibPath: stdlibPath(config),
      importDirs: config.get("importDirs") || [],
    },
  };
  client = new LanguageClient("okkama", "Okkama", serverOptions, clientOptions);
  try {
    await client.start();
  } catch (error) {
    vscode.window.showErrorMessage(
      `Okkama: не удалось запустить сервер языка (${command} lsp): ${error.message}. ` +
        "Укажите путь к Rust-сборке в настройке okkama.serverPath."
    );
  }
}

async function activate(context) {
  context.subscriptions.push(
    vscode.commands.registerCommand("okkama.restartServer", async () => {
      if (client) {
        await client.stop();
      }
      await start();
    })
  );
  await start();
}

function deactivate() {
  return client ? client.stop() : undefined;
}

module.exports = { activate, deactivate };
