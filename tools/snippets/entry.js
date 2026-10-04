import 'core-js/actual/url/index.js';
import 'core-js/actual/url-search-params/index.js';
import 'core-js/actual/btoa.js';
import 'core-js/actual/atob.js';
import { snippetz } from '@scalar/snippetz';
const engine = snippetz();
globalThis.moleapiSnippetCatalog = () => JSON.stringify(engine.clients().map(({ key, title, clients }) => ({target:key,title,clients:clients.map(({client,title}) => ({client,title}))})));
globalThis.moleapiSnippetGenerate = (input) => {
  const {target, client, request} = JSON.parse(input);
  const code = engine.print(target, client, request);
  if (typeof code !== 'string') throw new Error('Unsupported snippet target/library');
  return code;
};
