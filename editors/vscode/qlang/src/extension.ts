import * as fs from "node:fs";
import * as path from "node:path";
import * as vscode from "vscode";
import {
  Executable,
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

function serverBinaryName(): string {
  return process.platform === "win32" ? "qlsp.exe" : "qlsp";
}

function repoRoot(context: vscode.ExtensionContext): string {
  return path.resolve(context.extensionPath, "..", "..", "..");
}

function workspaceRoot(): string | undefined {
  return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
}

function isExistingFile(filePath: string): boolean {
  try {
    return fs.statSync(filePath).isFile();
  } catch {
    return false;
  }
}

function resolveConfiguredPath(
  rawPath: string,
  context: vscode.ExtensionContext
): string {
  if (path.isAbsolute(rawPath)) {
    return rawPath;
  }

  const baseDir = workspaceRoot() ?? context.extensionPath;
  return path.resolve(baseDir, rawPath);
}

function configuredServerExecutable(
  context: vscode.ExtensionContext
): Executable | undefined {
  const configuration = vscode.workspace.getConfiguration("qlang");
  const rawPath = configuration.get<string>("server.path")?.trim() ?? "";
  if (!rawPath) {
    return undefined;
  }

  const serverPath = resolveConfiguredPath(rawPath, context);
  const args = configuration.get<string[]>("server.args") ?? [];
  if (!isExistingFile(serverPath)) {
    throw new Error(
      `Configured qlang.server.path does not exist or is not a file: ${serverPath}`
    );
  }

  return {
    command: serverPath,
    args,
    options: {
      cwd: workspaceRoot() ?? repoRoot(context),
    },
  };
}

function autodetectedServerExecutable(
  context: vscode.ExtensionContext
): Executable {
  const configuration = vscode.workspace.getConfiguration("qlang");
  const args = configuration.get<string[]>("server.args") ?? [];
  const binaryName = serverBinaryName();
  const root = repoRoot(context);
  const candidates = [
    path.join(root, "target", "debug", binaryName),
    path.join(root, "target", "release", binaryName),
  ];

  const command =
    candidates.find((candidate) => isExistingFile(candidate)) ?? binaryName;

  return {
    command,
    args,
    options: {
      cwd: workspaceRoot() ?? root,
    },
  };
}

function serverExecutable(context: vscode.ExtensionContext): Executable {
  return (
    configuredServerExecutable(context) ?? autodetectedServerExecutable(context)
  );
}

function extensionVersion(context: vscode.ExtensionContext): string {
  const version = context.extension.packageJSON?.version;
  return typeof version === "string" && version.trim() ? version.trim() : "unknown";
}

function serverReportedVersion(client: LanguageClient): string | undefined {
  const version = client.initializeResult?.serverInfo?.version;
  return typeof version === "string" && version.trim() ? version.trim() : undefined;
}

function serverLocation(executable: Executable): string {
  return path.isAbsolute(executable.command)
    ? executable.command
    : `${executable.command} (from PATH)`;
}

function clientOptions(): LanguageClientOptions {
  return {
    documentSelector: [{ scheme: "file", language: "qlang" }],
    outputChannelName: "qlang",
  };
}

async function openExtensionReadme(
  context: vscode.ExtensionContext
): Promise<void> {
  const readmeUri = vscode.Uri.file(path.join(context.extensionPath, "README.md"));
  await vscode.commands.executeCommand("markdown.showPreview", readmeUri);
}

async function showStartError(
  context: vscode.ExtensionContext,
  error: unknown
): Promise<void> {
  const message =
    error instanceof Error
      ? error.message
      : "Failed to start qlsp. Check the server path and build state.";
  const selection = await vscode.window.showErrorMessage(
    `qlang: ${message}`,
    "Open Extension README",
    "Open Settings"
  );

  if (selection === "Open Extension README") {
    await openExtensionReadme(context);
  } else if (selection === "Open Settings") {
    await vscode.commands.executeCommand(
      "workbench.action.openSettings",
      "@ext:qlang.qlang qlang"
    );
  }
}

async function warnVersionMismatch(
  context: vscode.ExtensionContext,
  nextClient: LanguageClient,
  executable: Executable
): Promise<void> {
  const expectedVersion = extensionVersion(context);
  const actualVersion = serverReportedVersion(nextClient);
  if (!actualVersion || actualVersion === expectedVersion) {
    return;
  }

  const selection = await vscode.window.showWarningMessage(
    `qlang: extension ${expectedVersion} is connected to qlsp ${actualVersion} at ${serverLocation(
      executable
    )}. Rebuild or install matching artifacts, or point qlang.server.path at the matching qlsp binary.`,
    "Open Extension README",
    "Open Settings"
  );

  if (selection === "Open Extension README") {
    await openExtensionReadme(context);
  } else if (selection === "Open Settings") {
    await vscode.commands.executeCommand(
      "workbench.action.openSettings",
      "@ext:qlang.qlang qlang"
    );
  }
}

async function startClient(context: vscode.ExtensionContext): Promise<void> {
  if (client) {
    return;
  }

  const executable = serverExecutable(context);
  const serverOptions: ServerOptions = {
    run: executable,
    debug: executable,
  };
  const nextClient = new LanguageClient(
    "qlang",
    "qlang",
    serverOptions,
    clientOptions()
  );

  try {
    await nextClient.start();
    await warnVersionMismatch(context, nextClient, executable);
    client = nextClient;
  } catch (error) {
    await showStartError(context, error);
  }
}

async function stopClient(): Promise<void> {
  if (!client) {
    return;
  }

  const activeClient = client;
  client = undefined;
  await activeClient.stop();
}

async function restartClient(
  context: vscode.ExtensionContext,
  showMessage = false
): Promise<void> {
  await stopClient();
  await startClient(context);
  if (showMessage && client) {
    void vscode.window.showInformationMessage(
      "qlang: language server restarted."
    );
  }
}

export async function activate(
  context: vscode.ExtensionContext
): Promise<void> {
  context.subscriptions.push(
    vscode.commands.registerCommand("qlang.restartLanguageServer", async () => {
      await restartClient(context, true);
    })
  );

  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration(async (event) => {
      if (
        event.affectsConfiguration("qlang.server.path") ||
        event.affectsConfiguration("qlang.server.args")
      ) {
        await restartClient(context);
      }
    })
  );

  await startClient(context);
}

export async function deactivate(): Promise<void> {
  await stopClient();
}
