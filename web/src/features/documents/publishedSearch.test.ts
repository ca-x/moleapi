import {expect,it} from "vitest";
import {searchPublishedDocuments} from "./publishedSearch";

const documents=[
  {title:"ÄPI [one]",folder:"Manuals",source:"First 猫 example"},
  {title:"Other",folder:"Ärea",source:"[one] second"},
  {title:"Third",folder:null,source:"Unrelated"}
];

it("matches literal Unicode AND terms across title, folder and source in snapshot order",()=>{
  expect(searchPublishedDocuments(documents,"ä [one]")).toEqual([0,1]);
  expect(searchPublishedDocuments(documents,"猫 manuals")).toEqual([0]);
  expect(searchPublishedDocuments(documents,"second [one]")).toEqual([1]);
  expect(searchPublishedDocuments(documents,"[.*]")).toEqual([]);
  expect(searchPublishedDocuments(documents,"A\u0308PI")).toEqual([0]);
});

it("restores the full list for an empty query and never mutates source",()=>{
  const original=JSON.stringify(documents);
  expect(searchPublishedDocuments(documents," \n ")).toEqual([0,1,2]);
  expect(searchPublishedDocuments(documents,"not present")).toEqual([]);
  expect(JSON.stringify(documents)).toBe(original);
});
