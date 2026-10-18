// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import MarkdownPreview from "./MarkdownPreview";

afterEach(cleanup);

it("opens only current-workspace request links and keeps other destinations inert",()=>{
  const open=vi.fn();
  const source="[Open](moleapi://request/a%2Fb) [Missing](moleapi://request/other) [Bad](moleapi://request/a%2fb) [Safe](https://example.test) [Script](javascript:alert(1)) ![Remote](https://evil.test/i.png)";
  const page=render(<MarkdownPreview source={source} requestIds={new Set(["a/b"])} onOpenRequest={open}/>);
  fireEvent.click(screen.getByRole("button",{name:"Open"}));
  expect(open).toHaveBeenCalledExactlyOnceWith("a/b");
  expect(screen.queryByRole("button",{name:"Missing"})).toBeNull();
  expect(screen.queryByRole("button",{name:"Bad"})).toBeNull();
  expect(screen.queryByRole("link",{name:"Script"})).toBeNull();
  expect(screen.getByRole("link",{name:"Safe"}).getAttribute("rel")).toContain("noopener");
  expect(page.container.querySelector("img,script")).toBeNull();
  page.rerender(<MarkdownPreview source={source} requestIds={new Set()} onOpenRequest={open}/>);
  expect(screen.queryByRole("button",{name:"Open"})).toBeNull();
});
