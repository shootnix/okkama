// Run with vscode-textmate and vscode-oniguruma available through NODE_PATH.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const textmate = require('vscode-textmate');
const onig = require('vscode-oniguruma');
const root = path.resolve(__dirname, '..');
const failures = [];
let checks = 0;
(async () => {
  const wasm = fs.readFileSync(path.join(path.dirname(require.resolve('vscode-oniguruma/package.json')), 'release/onig.wasm'));
  await onig.loadWASM(wasm.buffer.slice(wasm.byteOffset, wasm.byteOffset + wasm.byteLength));
  const registry = new textmate.Registry({
    onigLib: Promise.resolve({createOnigScanner: p => new onig.OnigScanner(p), createOnigString: s => new onig.OnigString(s)}),
    loadGrammar: async scope => scope === 'source.okkama'
      ? textmate.parseRawGrammar(fs.readFileSync(path.join(root, 'syntaxes/okkama.tmLanguage.json'), 'utf8'), 'okkama.json') : null,
  });
  const grammar = await registry.loadGrammar('source.okkama');
  function tokenize(source) {
    let stack = textmate.INITIAL;
    return source.split('\n').map(line => {
      const result = grammar.tokenizeLine(line, stack);
      stack = result.ruleStack;
      return {line, tokens: result.tokens};
    });
  }
  function check(source, line, word, scope, absent = false) {
    checks++;
    const row = tokenize(source)[line];
    let offset = row.line.indexOf(word);
    while (offset >= 0 && ((/\w/.test(word[0]) && /\w/.test(row.line[offset - 1] || ' ')) ||
      (/\w/.test(word.at(-1)) && /\w/.test(row.line[offset + word.length] || ' ')))) {
      offset = row.line.indexOf(word, offset + 1);
    }
    const token = row.tokens.find(t => t.startIndex <= offset && t.endIndex >= offset + word.length);
    const found = offset >= 0 && token && token.scopes.includes(scope);
    try { assert.equal(Boolean(found), !absent, `${JSON.stringify(word)}: ${scope} in ${JSON.stringify(row.line)}; got ${JSON.stringify(token?.scopes)}`); }
    catch (error) { failures.push(error.message); }
  }
  check('let result = spawn Worker().run();', 0, 'spawn', 'keyword.control.async.okkama');
  check('return Customer(7);', 0, 'return', 'keyword.control.exception.okkama');
  check('let ratio is rat = 1.rat();', 0, 'rat', 'storage.type.primitive.okkama');
  check('save() is void {}', 0, 'void', 'storage.type.primitive.okkama');
  check('object Box<T: Reading> {}', 0, 'Reading', 'entity.name.type.okkama');
  check('object C { C(:name); renamed() is ^C { ^name="New"; } }', 0, '^', 'keyword.operator.successor.okkama');
  check('^name = "New";', 0, '^', 'keyword.operator.successor.okkama');
  check('let flags = a ^ b;', 0, '^', 'keyword.operator.bitwise.okkama');
  check('let n = 1_000;', 0, '1_000', 'constant.numeric.integer.okkama');
  check('let n = 1_000.25_5;', 0, '1_000.25_5', 'constant.numeric.float.okkama');
  check('let n = 0x7FFF_FFFF;', 0, '0x7FFF_FFFF', 'constant.numeric.integer.okkama');
  check('of Max(1,2)', 0, 'of', 'support.function.projection.okkama');
  check('view()', 0, 'view', 'keyword.declaration.okkama', true);
  check('cap()', 0, 'cap', 'keyword.declaration.cap.okkama', true);
  check('test.skip("later")', 0, 'skip', 'support.function.assertion.okkama');
  check('fake.writing.remove("key")', 0, 'remove', 'support.function.fake-state.okkama');
  for (const declaration of ['cap Card {', 'cap Customer::Card {', 'cap Box::lowercase {', 'cap\nCustomer::Card\n{']) {
    const source = declaration + '\n label is string { "}"; }\n}';
    check(source, 0, 'cap', 'keyword.declaration.cap.okkama');
    const fieldLine = declaration.split('\n').length;
    check(source, fieldLine, 'label', 'variable.other.property.cap.okkama');
  }
  const nested = 'cap Customer::Card {\n name is string {\n  if true { "}"; } else { "{ }"; }\n }\n callback is () is string { () is string { Customer::name; }; }\n}\nlet after is int = 1;';
  check(nested, 4, 'callback', 'variable.other.property.cap.okkama');
  check(nested, 6, 'after', 'meta.declaration.cap.okkama', true);
  check(nested, 4, 'name', 'variable.other.member.cap.okkama');
  check('let card is domain.Box<int>::lowercase = owner::lowercase;', 0, 'lowercase', 'entity.name.type.cap.okkama');
  check('let state is @Customer::Card = [customer::Card];', 0, 'Card', 'entity.name.type.cap.okkama');
  check('let state is %string:Box<@int>::Card;', 0, 'Card', 'entity.name.type.cap.okkama');
  check('let card is\n Box<int>::lowercase;', 1, 'lowercase', 'entity.name.type.cap.okkama');
  check('accept(operation is async (Customer::Card, @Box<int>::Named) is string) is void {}', 0, 'Named', 'entity.name.type.cap.okkama');
  check('cap Card {\n values is %string:Box<@int>::Named { source; }\n}', 1, 'Named', 'entity.name.type.cap.okkama');
  check('object C { C(:name); cap Card { label is string {name;} } nextMethod() is int {1;} }', 0, 'nextMethod', 'entity.name.function.okkama');
  check('cap Customer::Card { x is int {1;} }', 0, 'Customer', 'entity.name.type.okkama');
  check('cap Customer::Card { x is int {1;} }', 0, 'Card', 'entity.name.type.cap.okkama');
  check('repo.save(customer::Persistence);', 0, 'Persistence', 'variable.other.member.cap.okkama');
  check('customer::Public::email', 0, 'email', 'variable.other.member.cap.okkama');
  check('// cap Card {', 0, 'cap', 'keyword.declaration.cap.okkama', true);
  check('"cap Card {"', 0, 'cap', 'keyword.declaration.cap.okkama', true);
  check('```\ncap Card {\n```\nlet after is int = 1;', 1, 'cap', 'string.quoted.other.raw.fenced.okkama');
  check('cap Card {\n text is string { ```\n } cap Fake {\n ```; }\n nextValue is int { 1; }\n}', 4, 'nextValue', 'variable.other.property.cap.okkama');
  for (const file of ['examples/object-caps.ok',
    'toaster/encapsulation/tester-one.ok']) {
    const source = fs.readFileSync(path.join(root, '..', file), 'utf8');
    const rows = tokenize(source + '\n__okkama_end__');
    checks++;
    const last = rows.at(-1).tokens[0];
    try { assert.deepEqual(last.scopes, ['source.okkama'], `unclosed highlighting context in ${file}`); }
    catch (error) { failures.push(error.message); }
  }
  console.log(JSON.stringify({checks, passed: checks - failures.length, failed: failures.length, failures}, null, 2));
  registry.dispose();
  if (failures.length) process.exitCode = 1;
})().catch(error => { console.error(error); process.exitCode = 1; });
