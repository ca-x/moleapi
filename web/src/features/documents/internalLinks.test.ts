import {expect,it} from "vitest";
import {formatRequestLink,parseRequestLink} from "./internalLinks";

it("roundtrips one encoded request ID without decoding twice",()=>{
  expect(formatRequestLink("a/b %2F")).toBe("moleapi://request/a%2Fb%20%252F");
  expect(parseRequestLink("moleapi://request/a%2Fb%20%252F")).toBe("a/b %2F");
});

it.each([
  "moleapi://request/",
  "moleapi://request/a/b",
  "moleapi://request/a%2fb",
  "moleapi://request/a%",
  "moleapi://request/a?x=1",
  "moleapi://request/a#part",
  "moleapi://user@request/a",
  "moleapi://request:80/a",
  "moleapi://other/a",
  "https://example.test/a",
])("rejects noncanonical request link %s",value=>{
  expect(parseRequestLink(value)).toBeNull();
});
