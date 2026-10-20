// @vitest-environment jsdom
import { useLayoutEffect } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { Theme } from "@radix-ui/themes";
import { afterEach, expect, it, vi } from "vitest";
import { initialData } from "../../shared/model";
import { setLanguage } from "../../shared/i18n";
import SharedResourceActions from "./SharedResourceActions";
afterEach(cleanup);

it("cannot retarget a pending confirmation before passive selection effects run", async () => {
  await setLanguage("en");
  const data = initialData();data.collections[0].id = "root";
  data.collections[0].requests[0].id = "a";data.collections[0].requests[1].id = "b";
  const onChange = vi.fn();
  function Frame({ requestId }: { requestId: string }) {
    useLayoutEffect(() => {
      if (requestId === "b") [...document.querySelectorAll<HTMLButtonElement>("button")].find((button) => button.textContent === "Confirm delete")?.click();
    }, [requestId]);
    return <Theme><SharedResourceActions data={data} workspaceName="Team" collectionId="root" requestId={requestId} onChange={onChange} /></Theme>;
  }
  const page = render(<Frame requestId="a" />);
  fireEvent.click(screen.getByRole("button", { name: "Delete request" }));
  expect(screen.getByRole("alertdialog")).toBeTruthy();
  page.rerender(<Frame requestId="b" />);
  expect(onChange).not.toHaveBeenCalled();
});
