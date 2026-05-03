const fs = require("node:fs");
const path = require("node:path");
const assert = require("node:assert/strict");

const grammarPath = path.join(__dirname, "..", "syntaxes", "qlang.tmLanguage.json");
const grammar = JSON.parse(fs.readFileSync(grammarPath, "utf8"));

const patterns = grammar.patterns
  .flatMap((entry) => {
    const include = entry.include?.replace(/^#/, "");
    return grammar.repository[include]?.patterns ?? [];
  })
  .filter((pattern) => typeof pattern.match === "string")
  .map((pattern) => ({
    name: pattern.name,
    captures: pattern.captures,
    regex: new RegExp(pattern.match),
  }));

const cases = [
  ...["if", "else", "match", "for", "while", "loop", "in", "return", "break", "continue", "await", "spawn", "defer"]
    .map((keyword) => [keyword, "keyword.control.qlang"]),
  ...["pub", "unsafe", "move", "async"].map((keyword) => [keyword, "storage.modifier.qlang"]),
  ...["fn", "struct", "enum", "trait", "impl", "extend", "type", "opaque", "data", "extern", "where", "satisfies"]
    .map((keyword) => [keyword, "storage.type.qlang"]),
  ...["package", "let", "var", "const", "static", "use"].map((keyword) => [keyword, "keyword.declaration.qlang"]),
  ...["is", "as"].map((keyword) => [keyword, "keyword.operator.qlang"]),
  ...["true", "false"].map((keyword) => [keyword, "constant.language.boolean.qlang"]),
  ["none", "constant.language.none.qlang"],
  ["self", "storage.type.self.qlang"],
];

const nonKeywords = ["import"];
const nonBuiltins = ["Option", "Result"];

for (const [keyword, expectedScope] of cases) {
  const match = patterns.find((pattern) => pattern.regex.test(keyword));
  assert.equal(
    match?.name,
    expectedScope,
    `${keyword} should be tokenized as ${expectedScope}, got ${match?.name ?? "no match"}`
  );
}

for (const word of nonKeywords) {
  const match = patterns.find((pattern) => pattern.regex.test(word));
  assert.doesNotMatch(
    match?.name ?? "",
    /keyword|storage\.type|storage\.modifier|constant\.language/,
    `${word} should not be tokenized as a qlang lexer keyword`
  );
}

for (const word of nonBuiltins) {
  const match = patterns.find((pattern) => pattern.regex.test(word));
  assert.notEqual(
    match?.name,
    "support.type.builtin.qlang",
    `${word} is a stdlib type and should not be tokenized as a builtin`
  );
}

const opaqueDeclaration = patterns.find((pattern) => pattern.regex.test("opaque type UserId"));
assert.equal(
  opaqueDeclaration?.captures?.["3"]?.name,
  "entity.name.type.qlang",
  "opaque type declarations should capture the declared type name, not the `type` keyword"
);

console.log(`checked ${cases.length} qlang grammar keyword scopes`);
