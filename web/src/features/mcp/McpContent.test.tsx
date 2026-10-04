// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import McpContent from "./McpContent";
vi.mock("../../shared/ui",()=>({Editor:({value,label}:{value:string;label:string})=><textarea aria-label={label} value={value} readOnly/>}));
afterEach(cleanup);
it("renders embedded raster data but does not fetch resource links or execute HTML/SVG",()=>{
  render(<McpContent dark={false} result={{content:[{type:"image",mimeType:"image/png",data:"AA=="},{type:"image",mimeType:"image/svg+xml",data:"AA=="},{type:"resource_link",name:"external",uri:"https://example.test/private"},{type:"text",text:"<script>alert(1)</script>"}]}}/>);
  expect(screen.getAllByRole("img")).toHaveLength(1);
  expect(screen.getByRole("img").getAttribute("src")).toBe("data:image/png;base64,AA==");
  expect(screen.queryByRole("link")).toBeNull();
  expect(document.querySelector("script")).toBeNull();
});
