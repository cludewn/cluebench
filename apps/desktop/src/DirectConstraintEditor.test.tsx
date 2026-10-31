// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { useState } from "react";
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, createEvent, fireEvent, render, screen } from "@testing-library/react";
import DirectConstraintEditor from "./DirectConstraintEditor";
import { changeConstraintType, createDefaultDirectConstraint, validateDirectConstraints } from "./direct-constraints";
import type { ConstraintType, DirectConstraintDraft } from "./direct-constraints";

afterEach(cleanup);
function Harness({ initial = [] }: { initial?: DirectConstraintDraft[] }) {
  const [rows, setRows] = useState(initial);
  const [rejection, setRejection] = useState<string | null>(null);
  const [nextId, setNextId] = useState(Math.max(0, ...initial.map((row) => row.editorId)) + 1);
  return <><DirectConstraintEditor constraints={rows}
    onChange={(next) => { setRows(next); setRejection(null); }}
    onAdd={() => { setRows([...rows, createDefaultDirectConstraint(nextId)]); setNextId(nextId + 1); }} onRejected={setRejection} />
    {rejection && <p role="alert">{rejection}</p>}
    <button disabled={rejection !== null || validateDirectConstraints(rows).length > 0}>Submit</button>
    <output data-testid="rows">{JSON.stringify(rows)}</output></>;
}
const draftRows = () => JSON.parse(screen.getByTestId("rows").textContent ?? "[]") as DirectConstraintDraft[];
const add = () => fireEvent.click(screen.getByRole("button", { name: "Add condition" }));
const changeType = (type: ConstraintType, number = 1) => fireEvent.change(screen.getByLabelText(`Condition ${number} type`), { target: { value: type } });
const textField = (name: string) => screen.getByRole("textbox", { name }) as HTMLInputElement;
const change = (name: string, value: string) => fireEvent.change(textField(name), { target: { value } });
function transfer(target: Element, route: "paste" | "drop", raw: string) {
  const data = { types: ["text/plain"], getData: (type: string) => type === "text/plain" ? raw : "" };
  const event = route === "paste" ? createEvent.paste(target, { bubbles: true, cancelable: true, clipboardData: data })
    : createEvent.drop(target, { bubbles: true, cancelable: true, dataTransfer: data });
  fireEvent(target, event); return event;
}
const fields = [
  ["length", "minimum"], ["length", "maximum"], ["position", "position"], ["position", "allowed characters"],
  ["contains", "literal"], ["notContains", "literal"], ["prefix", "literal"], ["suffix", "literal"],
  ["characterCount", "minimum"], ["characterCount", "maximum"],
] as const;

describe("Direct condition editor (React/jsdom)", () => {
  it("starts empty and supports add, edit, enable, disable and remove", () => {
    render(<Harness />);
    expect(draftRows()).toEqual([]); add();
    change("Condition 1 minimum", "5");
    expect(draftRows()[0].constraint).toEqual({ type: "length", min: "5", max: "64" });
    fireEvent.click(screen.getByLabelText("Enable condition 1"));
    expect(draftRows()[0].enabled).toBe(false);
    expect(screen.getByText("Condition 1 — Disabled")).toBeTruthy();
    fireEvent.click(screen.getByLabelText("Enable condition 1"));
    expect(draftRows()[0].enabled).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Remove condition 1" }));
    expect(draftRows()).toEqual([]);
  });

  it("shows all seven type-specific editors and numeric text inputs", () => {
    render(<Harness />); add();
    for (const type of ["length", "position", "contains", "notContains", "prefix", "suffix", "characterCount"] as const) {
      changeType(type);
      expect(draftRows()[0].constraint.type).toBe(type);
      if (type === "length" || type === "characterCount") {
        expect(textField("Condition 1 minimum").type).toBe("text");
        expect(textField("Condition 1 minimum").inputMode).toBe("numeric");
        expect(textField("Condition 1 maximum").inputMode).toBe("numeric");
        if (type === "characterCount") {
          fireEvent.change(screen.getByLabelText("Condition 1 character class"), { target: { value: "symbol" } });
          expect(draftRows()[0].constraint).toMatchObject({ class: "symbol" });
          expect(screen.getByText(/Space is separate from symbol/)).toBeTruthy();
        }
      } else if (type === "position") {
        expect(textField("Condition 1 position").inputMode).toBe("numeric");
        expect(textField("Condition 1 allowed characters")).toBeTruthy();
      } else expect(textField("Condition 1 literal")).toBeTruthy();
    }
  });

  it("retains incomplete edits and invalid disabled rows until corrected or removed", () => {
    render(<Harness />); add(); change("Condition 1 minimum", "");
    expect(draftRows()[0].constraint).toMatchObject({ min: "" });
    fireEvent.click(screen.getByLabelText("Enable condition 1"));
    expect(screen.getByRole("button", { name: "Submit" }).hasAttribute("disabled")).toBe(true);
    change("Condition 1 minimum", "1x");
    expect(draftRows()[0].constraint).toMatchObject({ min: "1x" });
    change("Condition 1 minimum", "1");
    expect(screen.getByRole("button", { name: "Submit" }).hasAttribute("disabled")).toBe(false);
    expect(screen.getByText(/Correct or remove invalid disabled rows/)).toBeTruthy();
  });

  it("preserves confidence across Hard/Soft changes, including while disabled", () => {
    render(<Harness />); add();
    fireEvent.change(screen.getByLabelText("Condition 1 applicability"), { target: { value: "soft" } });
    fireEvent.change(screen.getByLabelText("Condition 1 confidence"), { target: { value: "high" } });
    fireEvent.click(screen.getByLabelText("Enable condition 1"));
    fireEvent.change(screen.getByLabelText("Condition 1 applicability"), { target: { value: "hard" } });
    expect(draftRows()[0]).toMatchObject({ applicability: "hard", confidence: "high", enabled: false });
    fireEvent.change(screen.getByLabelText("Condition 1 applicability"), { target: { value: "soft" } });
    expect(draftRows()[0]).toMatchObject({ applicability: "soft", confidence: "high", enabled: false });
    expect(screen.getByText(/Hard confidence has no effect/)).toBeTruthy();
  });

  it("keeps duplicate-valued rows distinct and preserves the final DOM field after middle removal", () => {
    render(<Harness />); add(); add(); add();
    const lastField = textField("Condition 3 minimum");
    const originalIds = draftRows().map((row) => row.editorId);
    expect(new Set(originalIds).size).toBe(3);
    fireEvent.click(screen.getByRole("button", { name: "Remove condition 2" }));
    expect(draftRows().map((row) => row.editorId)).toEqual([originalIds[0], originalIds[2]]);
    expect(textField("Condition 2 minimum")).toBe(lastField);
    add(); expect(draftRows()[2].editorId).toBeGreaterThan(originalIds[2]);
  });

  it("limits Add to 16 including disabled rows", () => {
    render(<Harness />);
    for (let index = 0; index < 16; index += 1) add();
    fireEvent.click(screen.getByLabelText("Enable condition 1"));
    expect(screen.getByRole("button", { name: "Add condition" }).hasAttribute("disabled")).toBe(true);
    expect(draftRows().length).toBe(16);
  });

  it.each(fields)("rejects raw Paste and Drop controls for %s/%s without altering the value", (type, name) => {
    render(<Harness initial={[changeConstraintType(createDefaultDirectConstraint(1), type)]} />);
    const field = textField(`Condition 1 ${name}`);
    const original = field.value;
    for (const route of ["paste", "drop"] as const) {
      for (const raw of ["Private\rPayload", "Private\nPayload", "Private\tPayload", "Private\u0000Payload", "Private\u007fPayload"]) {
        const event = transfer(field, route, raw);
        expect(event.defaultPrevented).toBe(true);
        expect(field.value).toBe(original);
        expect(screen.getByRole("alert").textContent).toBe("Input rejected. Direct condition fields cannot contain control characters.");
        expect(screen.getByRole("button", { name: "Submit" }).hasAttribute("disabled")).toBe(true);
      }
    }
  });

  it.each(fields)("checks native beforeinput data and dataTransfer for %s/%s", (type, name) => {
    render(<Harness initial={[changeConstraintType(createDefaultDirectConstraint(1), type)]} />);
    const field = textField(`Condition 1 ${name}`);
    const original = field.value;
    for (const route of ["data", "dataTransfer"] as const) {
      const event = new InputEvent("beforeinput", { bubbles: true, cancelable: true, inputType: "insertText", data: route === "data" ? "Private\rPayload" : "safe" });
      Object.defineProperty(event, "dataTransfer", { value: { getData: () => route === "dataTransfer" ? "Private\nPayload" : "safe" } });
      fireEvent(field, event);
      expect(event.defaultPrevented).toBe(true);
      expect(field.value).toBe(original);
      expect(screen.getByRole("alert").textContent).not.toContain("Private");
    }
  });

  it("rejects native line-break input without raw text", () => {
    render(<Harness initial={[createDefaultDirectConstraint(1)]} />);
    const event = new InputEvent("beforeinput", { bubbles: true, cancelable: true, inputType: "insertLineBreak" });
    fireEvent(textField("Condition 1 minimum"), event);
    expect(event.defaultPrevented).toBe(true);
  });

  it.each(fields)("restores the controlled value after noncancelable beforeinput and accepts a later valid edit for %s/%s", (type, name) => {
    render(<Harness initial={[changeConstraintType(createDefaultDirectConstraint(1), type)]} />);
    const field = textField(`Condition 1 ${name}`);
    const original = field.value;
    const event = new InputEvent("beforeinput", { bubbles: true, cancelable: false, inputType: "insertText", data: "Private\rPayload" });
    fireEvent(field, event);
    expect(event.defaultPrevented).toBe(false);
    // jsdom cannot execute native insertion; model only its subsequent sanitized edit.
    fireEvent.change(field, { target: { value: "PrivatePayload" } });
    expect(field.value).toBe(original);
    expect(screen.getByRole("alert").textContent).not.toContain("Private");
    fireEvent.change(field, { target: { value: "2" } });
    expect(field.value).toBe("2");
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("remounts guard state after type changes while preserving editor identity", () => {
    render(<Harness initial={[changeConstraintType(createDefaultDirectConstraint(91), "position")]} />);
    const oldField = textField("Condition 1 allowed characters");
    fireEvent(oldField, new InputEvent("beforeinput", { bubbles: true, cancelable: false, inputType: "insertText", data: "Private\rPayload" }));
    changeType("contains");
    const nextField = textField("Condition 1 literal");
    expect(nextField).not.toBe(oldField);
    fireEvent.change(nextField, { target: { value: " MiXeD " } });
    expect(draftRows()[0]).toMatchObject({ editorId: 91, constraint: { type: "contains", literal: " MiXeD " } });
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("permits valid raw transfers preserving case, spaces and duplicate characters", () => {
    render(<Harness initial={[changeConstraintType(createDefaultDirectConstraint(1), "position")]} />);
    const field = textField("Condition 1 allowed characters");
    const raw = " AaAA ";
    expect(transfer(field, "paste", raw).defaultPrevented).toBe(false);
    expect(transfer(field, "drop", raw).defaultPrevented).toBe(false);
    // Dispatching transfer alone has no insertion default in jsdom.
    fireEvent.change(field, { target: { value: raw } });
    expect(draftRows()[0].constraint).toMatchObject({ allowed: raw });
  });
});
