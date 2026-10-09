import {InputData} from 'quicktype-core/dist/esm/input/Inputs.js';
import {JSONSchemaInput} from 'quicktype-core/dist/esm/input/JSONSchemaInput.js';
import {quicktypeMultiFile} from 'quicktype-core/dist/esm/Run.js';
import {all} from 'quicktype-core/dist/esm/language/All.js';
import {openapiSchemaToJsonSchema} from '@openapi-contrib/openapi-schema-to-json-schema';

const names = new Set(['cs','c++','crystal','dart','elm','flow','go','haskell','java','javascript','kotlin','objc','pike','python','ruby','rust','swift','typescript']);
const languages = all.filter(language => names.has(language.name));
export const catalog = languages.map(language => ({
  id:`model-${language.name}`, kind:'model', title:language.displayName,
  upstream_stability:'upstream', validation:'engine-integration',
  options:Object.fromEntries([['schemaName','*'], ...language.optionDefinitions.map(option => [option.name,option.defaultValue])]),
  model_options:language.optionDefinitions.map(option => ({name:option.name,type:option.optionType,description:option.description,values:option.values ? Object.keys(option.values) : undefined})),
}));

globalThis.moleapiModelGenerate = async text => {
  const {target, specification, options} = JSON.parse(text);
  const language = languages.find(language => `model-${language.name}` === target);
  if (!language) throw new Error('Unknown model target');
  const schemas = specification.components?.schemas;
  if (!schemas || !Object.keys(schemas).length) throw new Error('No component models');
  const selected = options.schemaName ?? '*';
  const names = selected === '*' ? Object.keys(schemas) : [selected];
  if (names.length > 256 || names.some(name => !Object.hasOwn(schemas, name))) throw new Error('Invalid model selection');
  // The mature converter owns OpenAPI 3.0 nullable/required/type semantics.
  // Keep the document container so existing local component pointers resolve.
  const document = JSON.parse(JSON.stringify(specification));
  if (specification.openapi.startsWith('3.0.')) {
    for (const name of Object.keys(schemas)) document.components.schemas[name] = openapiSchemaToJsonSchema(schemas[name]);
  }
  const input = new JSONSchemaInput(undefined); // No filesystem or network schema store.
  for (const name of names) {
    const pointer = name.replaceAll('~','~0').replaceAll('/','~1');
    input.addSourceSync({name,schema:JSON.stringify(document),uris:[`moleapi-models.json#/components/schemas/${pointer}`]});
  }
  const inputData = new InputData();
  inputData.addInput(input);
  const {schemaName, ...rendererOptions} = options;
  const files = await quicktypeMultiFile({inputData,lang:language,rendererOptions,outputFilename:`models.${language.extension}`});
  return JSON.stringify(Object.fromEntries([...files].map(([path,result]) => [path,result.lines.join('\n')+'\n'])));
};
