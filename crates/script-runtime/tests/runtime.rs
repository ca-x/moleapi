use moleapi_core::*;
use moleapi_script_runtime::run;
use serde_json::json;
fn request() -> RequestSpec {
    serde_json::from_value(json!({"id":"r","name":"r","method":"GET","url":"https://example.com/","description":"","query":[],"headers":[],"body_kind":"none","body":"","auth":{"kind":"none","token":"","username":"","password":""},"timeout_ms":1000,"follow_redirects":true,"verify_tls":true,"assertions":[],"examples":[]})).unwrap()
}
fn evaluate(source: &str) -> anyhow::Result<moleapi_script_runtime::ScriptOutput> {
    run(
        &[source.into()],
        &request(),
        None,
        &VariableScopes::default(),
    )
}
#[test]
fn genuine_javascript_mutates_request_and_collects_tests() {
    let result = evaluate(
        r#"
      const values = [1,2,3].map(n => n * n);
      pm.variables.set('sum', values.reduce((a,b) => a+b,0));
      pm.environment.set('object', {a:1,b:[2,3]});
      pm.request.method = 'POST';
      pm.request.url = 'https://example.com/changed';
      pm.request.headers.upsert({key:'x-test',value:'yes'});
      pm.request.body.update({mode:'json',raw:'{"ok":true}'});
      console.log('computed',values);
      pm.test('math',() => pm.expect(values).to.deep.equal([1,4,9]));
      pm.test('object order',() => pm.expect({b:2,a:1}).to.eql({a:1,b:2}));
      pm.test('failure',() => pm.expect(14).to.equal(15));
    "#,
    )
    .unwrap();
    assert_eq!(result.request.method, "POST");
    assert_eq!(result.request.body_kind, "json");
    assert_eq!(result.updates[0].value.as_deref(), Some("14"));
    assert_eq!(
        result.updates[1].value.as_deref(),
        Some("{\"a\":1,\"b\":[2,3]}")
    );
    assert_eq!(result.logs[0].message, "computed [1,4,9]");
    assert!(result.tests[0].passed && result.tests[1].passed && !result.tests[2].passed);
}
#[test]
fn cooperative_interrupt_memory_and_output_limits_reject_scripts() {
    let start = std::time::Instant::now();
    assert!(evaluate("while(true) {}").is_err());
    assert!(start.elapsed() < std::time::Duration::from_secs(3));
    assert!(
        evaluate("const chunks=[]; while(true) chunks.push(new Array(100000).fill(42));").is_err()
    );
    assert!(evaluate("/^(a+)+$/.test('a'.repeat(100000)+'!');").is_err());
    assert!(evaluate("new ArrayBuffer(128 * 1024 * 1024);").is_err());
    assert!(evaluate("console.log('x'.repeat(70000));").is_err());
    assert!(evaluate("console.log('界'.repeat(23000));").is_err());
    assert!(evaluate("pm.variables.set('large','x'.repeat(1024*1024+1));").is_err());
    assert!(evaluate("pm.request.body.update('x'.repeat(5*1024*1024+1));").is_err());
}
#[test]
fn isolation_and_unsupported_apis_are_explicit() {
    evaluate("globalThis.leaked='yes'; pm.environment.set('secret','private');").unwrap();
    let result = evaluate("pm.test('isolation',() => {pm.expect(typeof leaked).to.equal('undefined'); pm.expect(pm.environment.get('secret')).to.be.undefined;});").unwrap();
    assert!(result.tests[0].passed);
    for source in [
        "pm.sendRequest('https://example.com')",
        "require('fs')",
        "fetch('https://example.com')",
        "pm.iterationData.set('x','y')",
        "pm.cookies.get('x')",
    ] {
        assert!(evaluate(source).is_err(), "{source}");
    }
    assert!(
        evaluate("Promise.resolve().then(() => pm.variables.set('x','y'));")
            .unwrap_err()
            .to_string()
            .contains("Asynchronous")
    );
}
#[test]
fn unknown_keys_and_prototype_names_are_real_variable_keys() {
    let mut scopes = VariableScopes::default();
    scopes
        .environment
        .insert("__proto__".into(), "initial".into());
    let initial = run(&["pm.test('initial key',()=>pm.expect(pm.environment.get('__proto__')).to.equal('initial'));".into()],&request(),None,&scopes).unwrap();
    assert!(initial.tests[0].passed);
    let output = evaluate("pm.test('missing',() => pm.expect(pm.environment.get('toString')).to.be.undefined); pm.environment.set('__proto__','safe'); pm.test('key',() => pm.expect(pm.environment.get('__proto__')).to.equal('safe'));").unwrap();
    assert!(output.tests.iter().all(|test| test.passed));
}
#[test]
fn detached_settled_promises_are_rejected_even_when_script_returns_a_number() {
    for source in [
        "(async()=>{throw new Error('async failed')})(); 42;",
        "(async()=>42)(); 42;",
        "new Promise(resolve => resolve(42)); 42;",
        "Promise.reject(new Error('detached rejection')); 42;",
        "pm.test('detached',()=>{ (async()=>42)(); }); 42;",
    ] {
        let result = evaluate(source);
        assert!(result.is_err(), "detached promise was accepted: {source}");
        assert!(result.unwrap_err().to_string().contains("Asynchronous"));
    }
}
#[test]
fn to_object_preserves_prototype_named_keys_in_every_scope() {
    let result = evaluate("for (const scope of [pm.variables,pm.globals,pm.environment,pm.collectionVariables]) { scope.set('__proto__','proto-value'); pm.test('own key',()=>{const object=scope.toObject();pm.expect(Object.prototype.hasOwnProperty.call(object,'__proto__')).to.be.true;pm.expect(object['__proto__']).to.equal('proto-value');}); }").unwrap();
    assert!(
        result.tests.iter().all(|test| test.passed),
        "{:?}",
        result.tests
    );
}
#[test]
fn negated_property_assertions_negate_the_complete_property_value_condition() {
    let result = evaluate("pm.test('different value',()=>pm.expect({x:1}).not.have.property('x',2));pm.test('matching value fails',()=>pm.expect({x:1}).not.have.property('x',1));pm.test('missing property',()=>pm.expect({x:1}).not.have.property('y'));pm.test('null missing',()=>pm.expect(null).not.have.property('x'));pm.test('matching positive',()=>pm.expect({x:1}).have.property('x',1));pm.test('different positive fails',()=>pm.expect({x:1}).have.property('x',2));").unwrap();
    let passed: Vec<_> = result.tests.iter().map(|test| test.passed).collect();
    assert_eq!(passed, vec![true, false, true, true, true, false]);
}
