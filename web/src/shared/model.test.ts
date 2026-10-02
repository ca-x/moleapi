import { describe, expect, it } from "vitest";
import { parse } from "shell-quote";
import { curlTemplate, newRequest, pair } from "./model";

describe("cURL copy template", () => {
  it("puts enabled encoded parameters in the URL before the fragment without changing option positions", () => {
    const request = newRequest(
      "Search",
      "https://example.com/items?existing=yes#details",
    );
    request.query = [
      pair("q", "a & b"),
      { ...pair("ignored", "value"), enabled: false },
    ];
    const args = parse(curlTemplate(request));
    expect(args.slice(0, 4)).toEqual([
      "curl",
      "-X",
      "GET",
      "https://example.com/items?existing=yes&q=a%20%26%20b#details",
    ]);
    expect(args).toContain("--max-time");
    expect(args).not.toContain("ignored");
  });
  it("round trips shell quotes and multiline body text without evaluating shell expressions", () => {
    const request = newRequest("Quoted", "https://example.com/O'Reilly");
    request.method = "POST";
    request.body_kind = "text";
    request.body = "it's literal\n$(touch /tmp/danger) `echo secret` $TOKEN";
    request.headers = [pair("X-Note", "one's note")];
    const args = parse(curlTemplate(request));
    expect(args[3]).toBe(request.url);
    expect(args[args.indexOf("--data-raw") + 1]).toBe(request.body);
    expect(args[args.indexOf("-H") + 1]).toBe("X-Note: one's note");
    expect(args.every((value) => typeof value === "string")).toBe(true);
  });
  it("redacts marked and known credentials in headers, query, URL userinfo and auth", () => {
    const request = newRequest(
      "Private",
      "https://user:private-url-password@example.com/?token=inline-secret&visible=yes",
    );
    request.query = [
      pair("custom", "marked-secret", true),
      pair("api_key", "query-secret"),
    ];
    request.headers = [
      pair("Authorization", "Bearer header-secret"),
      pair("Custom", "marked-header-secret", true),
      pair("X-Visible", "public"),
    ];
    request.auth = {
      kind: "basic",
      username: "real-user",
      password: "auth-secret",
      token: "",
    };
    const command = curlTemplate(request);
    for (const secret of [
      "private-url-password",
      "inline-secret",
      "marked-secret",
      "query-secret",
      "header-secret",
      "marked-header-secret",
      "real-user",
      "auth-secret",
    ])
      expect(command).not.toContain(secret);
    expect(command).toContain("{{REDACTED}}");
    expect(command).toContain("X-Visible: public");
    expect(parse(command)).toContain("{{USERNAME}}:{{PASSWORD}}");
  });
  it("retains body kind, redirects, TLS option and timeout in copied templates", () => {
    const request = newRequest("JSON", "https://example.com/");
    request.body_kind = "json";
    request.body = '{"ok":true}';
    request.follow_redirects = false;
    request.verify_tls = false;
    request.timeout_ms = 12500;
    const args = parse(curlTemplate(request));
    expect(args).toContain("Content-Type: application/json");
    expect(args).toContain("-k");
    expect(args).not.toContain("-L");
    expect(args[args.indexOf("--max-time") + 1]).toBe("12.5");
  });
});
