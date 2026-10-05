(input => {
  "use strict";
  const stringify = JSON.stringify.bind(JSON), parse = JSON.parse.bind(JSON);
  const source = input.request, scopes = input.scopes;
  const taint = __moleapiTaint;
  const uncertain = __moleapiPrivacyUncertain, privacyOverflow = __moleapiPrivacyOverflow, captureUrl = __moleapiCaptureUrl, captureQuery = __moleapiCaptureQuery;
  const captureBegin = __moleapiCaptureBegin, captureEnd = __moleapiCaptureEnd;
  const captureSafely = action => {
    captureBegin();let complete = false;
    try { const result = action();complete = true;return result; }
    finally { captureEnd(complete); }
  };
  let privacyVisits = 0;
  const privacyVisit = () => { if (++privacyVisits > 16384) privacyOverflow(); };
  for (const name of Object.keys(scopes)) scopes[name] = Object.assign(Object.create(null),scopes[name]);
  const logs = [], tests = [], updates = [];
  const MAX_VALUE = 1024 * 1024, MAX_OUTPUT = 64 * 1024;
  let outputBytes = 0;
  const unsupported = name => { throw new Error(`Unsupported pm API: ${name} (compatibility v1)`); };
  const checked = value => {
    const text = typeof value === "string" ? value : stringify(value);
    if (typeof text !== "string" || text.length > MAX_VALUE) throw new Error("Variable value exceeds 1 MiB or cannot be serialized");
    return text;
  };
  const bounded = value => {
    const text = String(value);
    outputBytes += text.length;
    if (outputBytes > MAX_OUTPUT) throw new Error("Script output exceeds 64 KiB");
    return text;
  };
  const api = (object, name) => new Proxy(object, {
    get(target, key) {
      if (typeof key === "symbol" || key === "then") return target[key];
      if (!(key in target)) return unsupported(`${name}.${key}`);
      return target[key];
    }
  });
  const has = (object,key) => Object.prototype.hasOwnProperty.call(object,key);
  const effective = () => Object.assign(Object.create(null), scopes.project, scopes.collection, scopes.environment, scopes.data, scopes.temporary);
  const store = (scope, fallback = false, readonly = false) => api({
    get: key => { const map = fallback ? effective() : scopes[scope]; return has(map,String(key)) ? map[String(key)] : undefined; },
    has: key => has(fallback ? effective() : scopes[scope],String(key)),
    toObject: () => Object.fromEntries(Object.entries(fallback ? effective() : scopes[scope])),
    set(key,value) {
      if (readonly) throw new Error("Execution data is read-only");
      key = String(key); value = checked(value);
      if (!key || key.length > 1024) throw new Error("Invalid variable key");
      return captureSafely(() => {
        captureCurrentRequest();
        taint(value);
        scopes[scope][key] = value;
        captureCurrentRequest();
        if (updates.length >= 1000) throw new Error("Script exceeds 1000 variable updates");
        updates.push({scope,key,value});
      });
    },
    unset(key) {
      if (readonly) throw new Error("Execution data is read-only");
      key = String(key);
      if (!key || key.length > 1024) throw new Error("Invalid variable key");
      return captureSafely(() => {
        captureCurrentRequest();
        delete scopes[scope][key];
        captureCurrentRequest();
        if (updates.length >= 1000) throw new Error("Script exceeds 1000 variable updates");
        updates.push({scope,key});
      });
    },
    clear() { for (const key of Object.keys(scopes[scope])) this.unset(key); },
    replaceIn(text) {
      let size = 0;
      text = String(text);
      if (text.length > 5 * MAX_VALUE) throw new Error("Resolved request exceeds expansion limit");
      const result = text.replace(/\{\{\s*([^{}]*?)\s*\}\}/g, (_, key) => {
        const value = effective()[key];
        if (value === undefined) throw new Error("Unsupported or unresolved variable");
        size += value.length;
        if (size > 5 * MAX_VALUE) throw new Error("Resolved request exceeds expansion limit");
        return value;
      });
      if (result.length > 5 * MAX_VALUE || result.includes("{{") || result.includes("}}")) throw new Error("Unsupported or unresolved variable or expansion limit");
      return result;
    }
  }, scope);
  const privacyResolve = value => {
    try { return store("temporary",true).replaceIn(value); }
    catch (error) {
      if (error?.message === "Unsupported or unresolved variable") { uncertain(); return undefined; }
      privacyOverflow();
    }
  };
  const captureHeader = pair => captureSafely(() => {
    privacyVisit();
    const keys = [pair.key.toLowerCase()];
    const resolvedKey = privacyResolve(pair.key);
    if (resolvedKey !== undefined) keys.push(resolvedKey.toLowerCase());
    const unknownKey = resolvedKey === undefined;
    if (unknownKey || pair.secret === true || keys.some(key => ["authorization","proxy-authorization","cookie","x-api-key"].includes(key))) {
      const values = [pair.value], resolvedValue = privacyResolve(pair.value);
      if (resolvedValue !== undefined) values.push(resolvedValue);
      for (const value of new Set(values)) {
        taint(value);
        const space = value.indexOf(" ");
        const scheme = space < 0 ? "" : value.slice(0,space).toLowerCase();
        if (keys.some(key => ["authorization","proxy-authorization"].includes(key)) || unknownKey && ["bearer","basic"].includes(scheme)) {
          if (space >= 0) taint(value.slice(space+1).trim());
        }
      }
    }
  });
  const captureCurrentUrl = () => {
    const targets = [source.url];
    if (source.protocol?.kind === "graphql" && typeof source.protocol.subscription_url === "string") targets.push(source.protocol.subscription_url);
    for (const target of targets) {
      privacyVisit();captureUrl(target);
      const resolved = privacyResolve(target);
      if (resolved !== undefined && resolved !== target) captureUrl(resolved);
    }
  };
  const captureQueryPair = pair => {
    privacyVisit();
    const rawSensitive = captureQuery(pair.key,pair.value,pair.secret === true);
    const key = privacyResolve(pair.key);
    const sensitive = rawSensitive || key === undefined || captureQuery(key,pair.value,pair.secret === true);
    if (sensitive) {
      if (key === undefined) taint(pair.value);
      const value = privacyResolve(pair.value);
      if (value !== undefined) taint(value);
    }
  };
  const captureAuth = () => {
    privacyVisit();
    for (const value of [source.auth.token,source.auth.password]) if (value) {
      taint(value); const resolved = privacyResolve(value);
      if (resolved !== undefined) taint(resolved);
    }
    const kind = privacyResolve(source.auth.kind);
    if (source.auth.password || kind === "basic") {
      const value = `${source.auth.username}:${source.auth.password}`;
      taint(value); const resolved = privacyResolve(value);
      if (resolved !== undefined) taint(resolved);
    }
  };
  const captureCurrentRequest = () => captureSafely(() => {
    if (source.headers.length > 1000 || source.query.length > 1000) privacyOverflow();
    captureAuth();captureCurrentUrl();
    for (const pair of source.headers) if (pair.enabled) captureHeader(pair);
    for (const pair of source.query) if (pair.enabled) captureQueryPair(pair);
  });
  const headerApi = (pairs, mutable) => api({
    get: key => {
      const pair = pairs.find(p => p.enabled && p.key.toLowerCase() === String(key).toLowerCase());
      if (mutable && pair) captureHeader(pair);
      return pair?.value;
    },
    has: key => pairs.some(p => p.enabled && p.key.toLowerCase() === String(key).toLowerCase()),
    toObject: () => Object.fromEntries(pairs.filter(p => p.enabled).map(p => {if (mutable) captureHeader(p); return [p.key,p.value];})),
    all: () => pairs.filter(p => p.enabled).map(p => {if (mutable) captureHeader(p); return {key:p.key,value:p.value};}),
    add(pair) {
      if (!mutable) throw new Error("Response headers are read-only");
      if (!pair || typeof pair.key !== "string" || typeof pair.value !== "string") throw new Error("Header requires key and value strings");
      if (pairs.length >= 1000) privacyOverflow();
      captureHeader(pair);
      pairs.push({id:`script-header-${pairs.length}`,key:pair.key,value:pair.value,enabled:true,secret:pair.secret === true ? true : undefined});
    },
    remove(key) {
      if (!mutable) throw new Error("Response headers are read-only");
      for (let i = pairs.length - 1; i >= 0; i--) if (pairs[i].key.toLowerCase() === String(key).toLowerCase()) {captureHeader(pairs[i]); pairs.splice(i,1);}
    },
    upsert(pair) { this.remove(pair.key); this.add(pair); }
  },"headers");
  const request = {};
  Object.defineProperties(request, {
    url: {get: () => captureSafely(() => {captureCurrentUrl(); return source.url;}), set: value => captureSafely(() => {captureCurrentRequest(); source.url = checked(value); captureCurrentRequest();})},
    method: {get: () => source.method, set: value => {source.method = checked(value);}},
    headers: {value: headerApi(source.headers,true)},
    body: {value: api({
      get raw() { return source.body; },
      set raw(value) { source.body = checked(value); if (source.body_kind === "none") source.body_kind = "text"; },
      get mode() { return source.body_kind; },
      update(value) {
        if (typeof value === "string") { source.body = value; if (source.body_kind === "none") source.body_kind = "text"; }
        else if (value && typeof value.raw === "string") { if (!["none","json","text","form","binary","multipart"].includes(value.mode || "text")) throw new Error("Unsupported request body mode"); source.body = value.raw; source.body_kind = value.mode || "text"; }
        else throw new Error("Unsupported request body update; use a string or {raw,mode}");
      },
      toString: () => source.body
    },"request.body")}
  });
  const describe = value => { try { return stringify(value) ?? String(value); } catch { return String(value); } };
  const deepEqual = (a,b,depth = 0) => {
    if (Object.is(a,b)) return true;
    if (depth > 128) throw new Error("Assertion nesting exceeds 128 levels");
    if (a === null || b === null || typeof a !== "object" || typeof b !== "object" || Array.isArray(a) !== Array.isArray(b)) return false;
    const keys = Object.keys(a);
    return keys.length === Object.keys(b).length && keys.every(key => has(b,key) && deepEqual(a[key],b[key],depth+1));
  };
  const expect = (actual, label) => {
    let negate = false, deep = false;
    let chain;
    const check = (passed, expected) => {
      if (negate ? passed : !passed) throw new Error(`${label || "Assertion"}: expected ${describe(actual)} ${negate ? "not " : ""}${expected}`);
      negate = false;
      return chain;
    };
    const methods = {
      equal: expected => check(deep ? deepEqual(actual,expected) : actual === expected,`to equal ${describe(expected)}`),
      eq: expected => check(actual === expected,`to equal ${describe(expected)}`),
      eql: expected => check(deepEqual(actual,expected),`to deeply equal ${describe(expected)}`),
      include: expected => check(actual != null && typeof actual.includes === "function" && actual.includes(expected),`to include ${describe(expected)}`),
      contain: expected => methods.include(expected),
      match: expected => check(expected instanceof RegExp && expected.test(actual),`to match ${expected}`),
      above: expected => check(actual > expected,`to be above ${expected}`),
      below: expected => check(actual < expected,`to be below ${expected}`),
      least: expected => check(actual >= expected,`to be at least ${expected}`),
      most: expected => check(actual <= expected,`to be at most ${expected}`),
      within: (low, high) => check(actual >= low && actual <= high,`to be within ${low}..${high}`),
      lengthOf: expected => check(actual?.length === expected,`to have length ${expected}`),
      a: expected => check(expected === "array" ? Array.isArray(actual) : typeof actual === expected,`to have type ${expected}`),
      an: expected => methods.a(expected),
      property(key, value) {
        const present = actual != null && has(Object(actual),key);
        const withValue = arguments.length > 1;
        const matches = present && (!withValue || (deep ? deepEqual(actual[key],value) : actual[key] === value));
        check(matches,`to have property ${key}${withValue ? ` equal ${describe(value)}` : ""}`);
        return expect(present ? actual[key] : undefined,label);
      },
      status: expected => check(actual === expected,`to have status ${expected}`)
    };
    chain = new Proxy(methods, {
      get(target,key) {
        if (key === "then") return undefined;
        if (["to","be","been","is","that","which","and","has","have","with","at","of","same"].includes(key)) return chain;
        if (key === "deep") { deep = true; return chain; }
        if (key === "not") { negate = !negate; return chain; }
        if (key === "true") return check(actual === true,"to be true");
        if (key === "false") return check(actual === false,"to be false");
        if (key === "null") return check(actual === null,"to be null");
        if (key === "undefined") return check(actual === undefined,"to be undefined");
        if (key === "exist") return check(actual != null,"to exist");
        if (key === "ok") return check(Boolean(actual),"to be truthy");
        if (key in target) return target[key];
        return unsupported(`expect.${String(key)}`);
      }
    });
    return chain;
  };
  const pm = {
    compatibilityVersion: "moleapi-pm/1",
    variables: store("temporary",true),
    iterationData: store("data",false,true),
    environment: store("environment"),
    globals: store("project"),
    collectionVariables: store("collection"),
    request: api(request,"request"),
    expect,
    test(name, callback) {
      if (tests.length >= 200) throw new Error("Script exceeds 200 tests");
      name = bounded(name);
      let passed = true, actual = "passed";
      try {
        if (typeof callback !== "function") throw new Error("pm.test requires a callback");
        const result = callback();
        if (result && typeof result.then === "function") throw new Error("Async test callbacks are unsupported");
      } catch (error) { passed = false; actual = bounded(error.message || String(error)); }
      tests.push({id:`script-${tests.length}`,name,passed,actual,expected:"passed"});
    },
    sendRequest: () => unsupported("sendRequest"),
    execution: api({},"execution")
  };
  if (input.response) {
    const response = input.response;
    pm.response = api({
      code: response.status, status: response.status_text,
      responseTime: response.elapsed_ms, responseSize: response.size_bytes,
      headers: headerApi(response.headers,false),
      text: () => response.body,
      json: () => parse(response.body),
      to: {have: {status: expected => expect(response.status).equal(expected)}, be: {get ok() { return expect(response.status >= 200 && response.status < 300).true; }}}
    },"response");
  } else Object.defineProperty(pm,"response",{get: () => { throw new Error("pm.response is only available after the network response"); }});
  Object.defineProperty(globalThis,"pm",{value:api(pm,"pm"),writable:false,configurable:false});
  Object.defineProperty(globalThis,"console",{value: Object.fromEntries(["log","info","warn","error","debug"].map(level => [level,(...args) => {
    if (logs.length >= 200) throw new Error("Script exceeds 200 log entries");
    logs.push({level,message:bounded(args.map(v => typeof v === "string" ? v : describe(v)).join(" "))});
  }])),writable:false});
  for (const name of ["require","fetch","setTimeout","setInterval","queueMicrotask"]) globalThis[name] = () => unsupported(name);
  captureCurrentRequest();
  const exportState = () => stringify({request:source,logs,tests,updates});
  Object.defineProperty(globalThis,"__moleapiExport",{value:exportState,writable:false,configurable:false});
})
