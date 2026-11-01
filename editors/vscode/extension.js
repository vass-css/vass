/**
 * VASS Language Support for VS Code — zero-dependency LSP client.
 *
 * Spawns `vass lsp --stdio` (JSON-RPC over stdio) and wires it to:
 *   - publishDiagnostics  -> Problems panel (errors + warnings)
 *   - completion          -> IntelliSense (variables, mixins, functions, CSS)
 *   - hover, definition   -> docs & go-to-definition
 *   - documentSymbol      -> outline view
 *   - formatting          -> document format provider
 *
 * No npm packages are required: the LSP framing is implemented below, so the
 * extension works on a clean machine without `npm install`.
 */
'use strict';

const vscode = require('vscode');
const { spawn } = require('child_process');
const fs = require('fs');
const path = require('path');

const LANGUAGES = ['vass', 'vcss'];

/** Minimal JSON-RPC client over a child process' stdio. */
class LspClient {
  constructor(cmd, args, cwd, trace) {
    this.cmd = cmd;
    this.args = args;
    this.cwd = cwd;
    this.trace = trace || (() => {});
    this.nextId = 1;
    this.pending = new Map(); // id -> {resolve, reject}
    this.buffer = Buffer.alloc(0);
    this.notificationHandlers = new Map();
    this.requestHandlers = new Map();
    this.closed = false;
    this.exitListeners = [];
    this.retriedNoCwd = false;
    // A non-existent cwd makes Node report `spawn <cmd> ENOENT` even when the
    // executable exists — VS Code's per-workspace extension storage folder is
    // NOT pre-created by VS Code. Create it so the cwd is always valid.
    if (cwd) {
      try { fs.mkdirSync(cwd, { recursive: true }); } catch (e) { /* best effort */ }
    }
    this.spawnChild(cwd);
  }

  spawnChild(cwd) {
    this.child = spawn(this.cmd, this.args, { cwd, env: process.env, windowsHide: true });
    this.child.stdout.on('data', (chunk) => this.onData(chunk));
    this.child.stderr.on('data', (chunk) => this.trace('server stderr', chunk.toString()));
    this.child.on('error', (err) => {
      // ENOENT can also mean the cwd vanished after the mkdir. Retry once
      // without a cwd — the child then inherits the extension host's cwd.
      if (!this.retriedNoCwd && cwd && /ENOENT/.test(String(err.message))) {
        this.retriedNoCwd = true;
        this.trace('spawn-retry', `${err.message} — retrying without cwd`);
        this.closed = false;
        this.spawnChild(undefined);
        return;
      }
      this.rejectAll(err);
    });
    this.child.on('exit', (code, signal) => {
      this.closed = true;
      this.rejectAll(new Error(`server exited (code=${code}, signal=${signal})`));
      for (const fn of this.exitListeners) fn(code, signal);
    });
  }

  onExit(fn) { this.exitListeners.push(fn); }

  rejectAll(err) {
    for (const [, p] of this.pending) p.reject(err);
    this.pending.clear();
  }

  /** Accumulate bytes and dispatch complete `Content-Length` frames. */
  onData(chunk) {
    this.buffer = Buffer.concat([this.buffer, chunk]);
    for (;;) {
      const headerEnd = this.buffer.indexOf('\r\n\r\n');
      if (headerEnd < 0) return;
      const header = this.buffer.slice(0, headerEnd).toString('utf8');
      const match = /Content-Length: *(\d+)/i.exec(header);
      if (!match) {
        // Malformed frame; drop it to avoid wedging the stream.
        this.buffer = this.buffer.slice(headerEnd + 4);
        continue;
      }
      const length = parseInt(match[1], 10);
      const bodyStart = headerEnd + 4;
      if (this.buffer.length < bodyStart + length) return;
      const body = this.buffer.slice(bodyStart, bodyStart + length).toString('utf8');
      this.buffer = this.buffer.slice(bodyStart + length);
      this.dispatch(JSON.parse(body));
    }
  }

  dispatch(msg) {
    this.trace('recv', JSON.stringify(msg).slice(0, 400));
    if (msg.id !== undefined && (msg.method === undefined || msg.result !== undefined || msg.error !== undefined)) {
      const p = this.pending.get(msg.id);
      if (p) {
        this.pending.delete(msg.id);
        if (msg.error) p.reject(new Error(msg.error.message || 'server error'));
        else p.resolve(msg.result);
      }
      return;
    }
    if (msg.method && msg.id !== undefined) {
      // Server -> client request (e.g. workspace/configuration).
      const handler = this.requestHandlers.get(msg.method);
      const result = handler ? handler(msg.params) : null;
      this.rawSend({ jsonrpc: '2.0', id: msg.id, result: result === undefined ? null : result });
      return;
    }
    if (msg.method) {
      const handler = this.notificationHandlers.get(msg.method);
      if (handler) handler(msg.params);
    }
  }

  rawSend(obj) {
    const body = Buffer.from(JSON.stringify(obj), 'utf8');
    const header = Buffer.from(`Content-Length: ${body.length}\r\n\r\n`, 'utf8');
    this.trace('send', body.toString('utf8').slice(0, 400));
    return new Promise((resolve, reject) => {
      this.child.stdin.write(Buffer.concat([header, body]), (err) => (err ? reject(err) : resolve()));
    });
  }

  request(method, params) {
    if (this.closed) return Promise.reject(new Error('connection closed'));
    const id = this.nextId++;
    const p = new Promise((resolve, reject) => this.pending.set(id, { resolve, reject }));
    this.rawSend({ jsonrpc: '2.0', id, method, params });
    return p;
  }

  notify(method, params) {
    if (this.closed) return;
    this.rawSend({ jsonrpc: '2.0', method, params });
  }

  onNotification(method, handler) { this.notificationHandlers.set(method, handler); }
  onRequest(method, handler) { this.requestHandlers.set(method, handler); }

  dispose() {
    if (this.closed) return;
    this.closed = true;
    try { this.child.stdin.end(); } catch (e) { /* already closed */ }
    const t = setTimeout(() => { try { this.child.kill('SIGKILL'); } catch (e) { /* gone */ } }, 1500);
    this.child.on('exit', () => clearTimeout(t));
    try { this.child.kill(); } catch (e) { /* gone */ }
  }
}

/** Convert a LSP range/position (UTF-16) to vscode objects (also UTF-16). */
function toVsRange(range) {
  return new vscode.Range(
    Math.max(0, range.start.line), Math.max(0, range.start.character),
    Math.max(0, range.end.line), Math.max(0, range.end.character)
  );
}
function toLspPosition(pos) { return { line: pos.line, character: pos.character }; }
function toLspRange(range) {
  return { start: toLspPosition(range.start), end: toLspPosition(range.end) };
}
function docId(doc) {
  return { uri: String(doc.uri), languageId: doc.languageId, version: doc.version, text: doc.getText() };
}

class VassLspSession {
  constructor(context, output) {
    this.context = context;
    this.output = output;
    this.client = null;
    this.statusItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 10);
    this.diagnostics = vscode.languages.createDiagnosticCollection('vass');
    context.subscriptions.push(this.statusItem, this.diagnostics);
  }

  log(msg) { this.output.appendLine(`[${new Date().toLocaleTimeString()}] ${msg}`); }

  serverArgs() {
    const cfg = vscode.workspace.getConfiguration('vass');
    const args = cfg.get('server.args');
    return Array.isArray(args) && args.length ? args.map(String) : ['lsp', '--stdio'];
  }

  async start() {
    const cfg = vscode.workspace.getConfiguration('vass');
    const cmd = cfg.get('server.path') || 'vass';
    this.statusItem.text = '$(sync~spin) VASS: starting…';
    this.statusItem.show();
    const traceLevel = cfg.get('trace.server') || 'off';
    const trace = traceLevel === 'off'
      ? () => {}
      : (dir, msg) => this.log(`trace ${dir}: ${msg}`);

    let client;
    try {
      client = new LspClient(cmd, this.serverArgs(), this.context.storageUri?.fsPath || undefined, trace);
    } catch (err) {
      this.failToStart(cmd, err);
      return;
    }

    client.onExit((code) => {
      this.log(`server exited (code=${code})`);
      this.statusItem.text = '$(circle-slash) VASS: server stopped';
      if (code && code !== 0 && !this.disposing) {
        vscode.window.showErrorMessage(
          `VASS language server exited with code ${code}. ` +
          'Check the "VASS Language Server" output channel for details.'
        );
      }
    });

    client.onNotification('textDocument/publishDiagnostics', (params) => {
      const diags = (params.diagnostics || []).map((d) => new vscode.Diagnostic(
        toVsRange(d.range),
        d.message || 'unknown diagnostic',
        d.severity === 2 ? vscode.DiagnosticSeverity.Warning
          : d.severity === 3 ? vscode.DiagnosticSeverity.Information
          : d.severity === 4 ? vscode.DiagnosticSeverity.Hint
          : vscode.DiagnosticSeverity.Error
      ));
      this.diagnostics.set(vscode.Uri.parse(params.uri), diags);
    });

    client.onRequest('workspace/configuration', () =>
      LANGUAGES.map(() => vscode.workspace.getConfiguration('vass')));
    client.onRequest('client/registerCapability', () => {});
    client.onRequest('window/workDoneProgress/create', () => {});
    client.notify('window/workDoneProgress/create', { token: 'vass-init' });

    try {
      const result = await client.request('initialize', {
        processId: process.pid,
        rootPath: vscode.workspace.workspaceFolders?.[0]?.uri.fsPath || null,
        rootUri: vscode.workspace.workspaceFolders?.[0]?.uri.toString() || null,
        capabilities: {
          textDocument: {
            synchronization: { dynamicRegistration: false, didSave: true },
            completion: { completionItem: { snippetSupport: true } },
            hover: { contentFormat: ['markdown', 'plaintext'] },
            documentSymbol: { symbolKind: { valueSet: [] } },
            publishing: {},
          },
          workspace: { configuration: true },
        },
        initializationOptions: {},
      });
      const name = result?.serverInfo?.name || 'vass-lsp-server';
      const version = result?.serverInfo?.version || '';
      this.log(`initialized ${name} ${version} (spawned: ${cmd} ${this.serverArgs().join(' ')})`);
      client.notify('initialized', {});
      this.client = client;
      this.statusItem.text = `$(check) VASS ${version}`;
      this.statusItem.tooltip = `VASS language server running (${name} ${version})`;
      // Re-open already visible documents so diagnostics appear immediately.
      for (const editor of vscode.window.visibleTextEditors) this.openDocument(editor.document);
    } catch (err) {
      this.failToStart(cmd, err);
      client.dispose();
    }
  }

  failToStart(cmd, err) {
    this.log(`failed to start server: ${err.message}`);
    this.statusItem.text = '$(circle-slash) VASS: server failed';
    this.statusItem.show();
    const isWindows = process.platform === 'win32';
    vscode.window.showErrorMessage(
      `Could not start the VASS language server ("${cmd}"). ${err.message}`,
      'Open Output'
    ).then((pick) => {
      if (pick === 'Open Output') this.output.show();
    });
    this.log(
      'Hint: install VASS and/or set "vass.server.path" to the full vass executable path' +
      (isWindows ? ' (e.g. D:\\\\Tools\\\\vass\\\\vass.exe).' : ' (e.g. /usr/local/bin/vass).')
    );
  }

  active(doc) { return doc && LANGUAGES.includes(doc.languageId) && this.client; }

  openDocument(doc) {
    if (!this.active(doc)) return;
    this.client.notify('textDocument/didOpen', { textDocument: docId(doc) });
  }

  changeDocument(doc) {
    if (!this.active(doc)) return;
    this.client.notify('textDocument/didChange', {
      textDocument: { uri: String(doc.uri), version: doc.version },
      contentChanges: [{ text: doc.getText() }],
    });
  }

  saveDocument(doc) {
    if (!this.active(doc)) return;
    this.client.notify('textDocument/didSave', { textDocument: { uri: String(doc.uri) } });
  }

  closeDocument(doc) {
    if (!doc || !LANGUAGES.includes(doc.languageId)) return;
    if (this.client) this.client.notify('textDocument/didClose', { textDocument: { uri: String(doc.uri) } });
    this.diagnostics.delete(doc.uri);
  }

  warnFormatterUnavailable(doc) {
    if (this.formatWarned) return;
    this.formatWarned = true;
    this.log(`format requested for ${doc.uri} but the language server is not running`);
    vscode.window.showWarningMessage(
      'VASS formatter is unavailable because the language server is not running. ' +
      'Make sure `vass --version` works in a terminal, then run "VASS: Restart Language Server".',
      'Open Output'
    ).then((pick) => { if (pick === 'Open Output') this.output.show(); });
  }

  stop() {
    if (this.client) {
      const c = this.client;
      this.client = null;
      c.request('shutdown', null).catch(() => {}).finally(() => {
        c.notify('exit', null);
        c.dispose();
      });
    }
  }

  restart() {
    this.stop();
    this.diagnostics.clear();
    return this.start();
  }

  dispose() {
    this.disposing = true;
    this.stop();
    this.diagnostics.dispose();
    this.statusItem.dispose();
  }
}

async function activate(context) {
  const output = vscode.window.createOutputChannel('VASS Language Server');
  context.subscriptions.push(output);
  const session = new VassLspSession(context, output);
  context.subscriptions.push(session);

  // --- document lifecycle -------------------------------------------------
  context.subscriptions.push(vscode.workspace.onDidOpenTextDocument((d) => session.openDocument(d)));
  context.subscriptions.push(vscode.workspace.onDidChangeTextDocument((e) => session.changeDocument(e.document)));
  context.subscriptions.push(vscode.workspace.onDidSaveTextDocument((d) => session.saveDocument(d)));
  context.subscriptions.push(vscode.workspace.onDidCloseTextDocument((d) => session.closeDocument(d)));

  // --- providers ----------------------------------------------------------
  const selector = LANGUAGES.map((lang) => ({ language: lang, scheme: 'file' }))
    .concat(LANGUAGES.map((lang) => ({ language: lang, scheme: 'untitled' })));

  const callServer = async (method, doc, posOrParams, map) => {
    if (!session.active(doc)) return null;
    try {
      const params = posOrParams && posOrParams.position
        ? posOrParams
        : { textDocument: { uri: String(doc.uri) }, position: toLspPosition(posOrParams) };
      const result = await session.client.request(method, params);
      return map ? map(result) : result;
    } catch (err) {
      session.log(`${method} failed: ${err.message}`);
      return null;
    }
  };

  context.subscriptions.push(vscode.languages.registerCompletionItemProvider(selector, {
    provideCompletionItems: (doc, position) =>
      callServer('textDocument/completion', doc, position, (res) => {
        const items = Array.isArray(res) ? res : (res && res.items) || [];
        return items.map((it) => {
          const item = new vscode.CompletionItem(it.label, kindOf(it.kind));
          if (it.detail) item.detail = it.detail;
          if (it.documentation) item.documentation = typeof it.documentation === 'string'
            ? new vscode.MarkdownString(it.documentation) : it.documentation;
          if (it.insertText) item.insertText = new vscode.SnippetString(it.insertText);
          if (it.sortText) item.sortText = it.sortText;
          return item;
        });
      }),
  }, '$', '.', '@', '-'));

  context.subscriptions.push(vscode.languages.registerHoverProvider(selector, {
    provideHover: (doc, position) =>
      callServer('textDocument/hover', doc, position, (res) => {
        if (!res) return null;
        const contents = Array.isArray(res.contents) ? res.contents : [res.contents];
        const md = contents.map((c) => (typeof c === 'string' ? c : c.value || '')).join('\n\n');
        return new vscode.Hover(new vscode.MarkdownString(md), toVsRange(res.range || {
          start: { line: position.line, character: position.character },
          end: { line: position.line, character: position.character },
        }));
      }),
  }));

  context.subscriptions.push(vscode.languages.registerDefinitionProvider(selector, {
    provideDefinition: (doc, position) =>
      callServer('textDocument/definition', doc, position, (res) => {
        if (!res) return null;
        const list = Array.isArray(res) ? res : [res];
        return list.map((loc) => new vscode.Location(
          vscode.Uri.parse(loc.uri || (loc.targetUri && loc.targetUri)),
          toVsRange(loc.range || loc.targetSelectionRange || loc.range)
        )).filter((l) => l.uri);
      }),
  }));

  context.subscriptions.push(vscode.languages.registerDocumentSymbolProvider(selector, {
    provideDocumentSymbols: (doc) =>
      callServer('textDocument/documentSymbol', doc, { textDocument: { uri: String(doc.uri) } }, (res) => {
        if (!Array.isArray(res)) return [];
        return res.map((s) => {
          if (s.range && s.selectionRange) {
            // Hierarchical DocumentSymbol
            return new vscode.DocumentSymbol(
              s.name, s.detail || '', symbolKind(s.kind),
              toVsRange(s.range), toVsRange(s.selectionRange),
              (s.children || []).map((c) => new vscode.DocumentSymbol(
                c.name, c.detail || '', symbolKind(c.kind),
                toVsRange(c.range), toVsRange(c.selectionRange)
              ))
            );
          }
          // Flat SymbolInformation
          return new vscode.SymbolInformation(
            s.name, symbolKind(s.kind), s.containerName || '',
            new vscode.Location(doc.uri, toVsRange(s.location ? s.location.range : s.range))
          );
        });
      }),
  }));

  context.subscriptions.push(vscode.languages.registerDocumentFormattingEditProvider(selector, {
    provideDocumentFormattingEdits: (doc) => {
      if (!session.active(doc)) {
        // Never fail silently on Ctrl+S — tell the user why nothing happened.
        session.warnFormatterUnavailable(doc);
        return [];
      }
      return callServer('textDocument/formatting', doc, {
        textDocument: { uri: String(doc.uri) },
        options: { tabSize: (doc.options || {}).tabSize || 2, insertSpaces: (doc.options || {}).insertSpaces !== false },
      }, (res) => (Array.isArray(res) ? res.map((e) => new vscode.TextEdit(toVsRange(e.range), e.newText)) : []));
    },
  }));

  // --- commands -----------------------------------------------------------
  context.subscriptions.push(vscode.commands.registerCommand('vass.enableFormatOnSave', async () => {
    const editorCfg = vscode.workspace.getConfiguration('editor');
    const target = vscode.ConfigurationTarget.Global;
    for (const lang of LANGUAGES) {
      await editorCfg.update('formatOnSave', true, target, lang);
      await editorCfg.update('defaultFormatter', 'basslang.vass-vscode', target, lang);
    }
    vscode.window.showInformationMessage(
      'VASS: format-on-save enabled for [vass] and [vcss] using the VASS formatter.'
    );
  }));

  context.subscriptions.push(vscode.commands.registerCommand('vass.restartServer', async () => {
    await session.restart();
    vscode.window.showInformationMessage('VASS language server restarted.');
  }));

  context.subscriptions.push(vscode.commands.registerCommand('vass.showSetupInfo', () => {
    const cfg = vscode.workspace.getConfiguration('vass');
    const cmdPath = cfg.get('server.path');
    const args = session.serverArgs();
    const msg =
      `VASS extension ${require('./package.json').version}\n` +
      `Server command: ${cmdPath} ${args.join(' ')}\n\n` +
      'If the server does not start:\n' +
      '1. Install VASS and make sure `vass --version` works in a terminal.\n' +
      '2. Or point Settings > VASS > Server: Path to the full vass executable.\n' +
      '3. Run "VASS: Restart Language Server".\n' +
      'Server output is in the "VASS Language Server" output channel.';
    vscode.window.showInformationMessage(msg, 'Open Output').then((pick) => {
      if (pick === 'Open Output') output.show();
    });
  }));

  output.appendLine(`VASS extension activating (server: ${cfgPath()} ${session.serverArgs().join(' ')})`);
  await session.start();

  // One-time, consent-based hint: offer to turn on format-on-save for both
  // languages when saving would silently not format (the classic "Ctrl+S
  // kaj kore na" setup issue).
  let formatHintShown = false;
  const maybeOfferFormatOnSave = (doc) => {
    if (formatHintShown || !doc || !LANGUAGES.includes(doc.languageId)) return;
    formatHintShown = true;
    const cfg = vscode.workspace.getConfiguration('editor', doc);
    if (cfg.get('formatOnSave') && cfg.get('defaultFormatter') === 'basslang.vass-vscode') return;
    vscode.window.showInformationMessage(
      'VASS: enable format-on-save for .vass and .vcss files?',
      'Enable', 'Not now'
    ).then((pick) => {
      if (pick === 'Enable') vscode.commands.executeCommand('vass.enableFormatOnSave');
    });
  };
  context.subscriptions.push(vscode.workspace.onDidOpenTextDocument(maybeOfferFormatOnSave));
  for (const editor of vscode.window.visibleTextEditors) maybeOfferFormatOnSave(editor.document);

  function cfgPath() { return vscode.workspace.getConfiguration('vass').get('server.path') || 'vass'; }
}

function kindOf(k) {
  // LSP CompletionItemKind -> vscode.CompletionItemKind (matching subset)
  const map = {
    3: vscode.CompletionItemKind.Function, 6: vscode.CompletionItemKind.Variable,
    7: vscode.CompletionItemKind.Class, 9: vscode.CompletionItemKind.Module,
    10: vscode.CompletionItemKind.Property, 12: vscode.CompletionItemKind.Value,
    13: vscode.CompletionItemKind.Enum, 14: vscode.CompletionItemKind.Keyword,
    21: vscode.CompletionItemKind.Constant,
  };
  return map[k] || vscode.CompletionItemKind.Text;
}

function symbolKind(k) {
  // LSP SymbolKind -> vscode.SymbolKind (identity for the common range 1-26)
  const map = {
    1: vscode.SymbolKind.File, 2: vscode.SymbolKind.Module, 3: vscode.SymbolKind.Namespace,
    4: vscode.SymbolKind.Package, 5: vscode.SymbolKind.Class, 6: vscode.SymbolKind.Method,
    7: vscode.SymbolKind.Property, 8: vscode.SymbolKind.Field, 9: vscode.SymbolKind.Constructor,
    10: vscode.SymbolKind.Enum, 11: vscode.SymbolKind.Interface, 12: vscode.SymbolKind.Function,
    13: vscode.SymbolKind.Variable, 14: vscode.SymbolKind.Constant, 15: vscode.SymbolKind.String,
    16: vscode.SymbolKind.Number, 17: vscode.SymbolKind.Boolean, 18: vscode.SymbolKind.Array,
    25: vscode.SymbolKind.TypeParameter,
  };
  return map[k] || vscode.SymbolKind.Variable;
}

function deactivate() {}

module.exports = { activate, deactivate };
