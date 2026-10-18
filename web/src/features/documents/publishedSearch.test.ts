import {expect,it} from "vitest";
import {indexPublishedDocuments,searchPublishedDocuments} from "./publishedSearch";

const documents=[
  {title:"ÄPI [one]",folder:"Manuals",source:"First 猫 example"},
  {title:"Other",folder:"Ärea",source:"[one] second"},
  {title:"Third",folder:null,source:"Unrelated"}
];

it("matches literal Unicode AND terms across title, folder and source in snapshot order",()=>{
  const index=indexPublishedDocuments(documents);
  expect(searchPublishedDocuments(index,"ä [one]")).toEqual([0,1]);
  expect(searchPublishedDocuments(index,"猫 manuals")).toEqual([0]);
  expect(searchPublishedDocuments(index,"second [one]")).toEqual([1]);
  expect(searchPublishedDocuments(index,"[.*]")).toEqual([]);
  expect(searchPublishedDocuments(index,"A\u0308PI")).toEqual([0]);
});

it("restores the full list for an empty query and never mutates source",()=>{
  const original=JSON.stringify(documents);
  const index=indexPublishedDocuments(documents);
  expect(searchPublishedDocuments(index," \n ")).toEqual([0,1,2]);
  expect(searchPublishedDocuments(index,"not present")).toEqual([]);
  expect(JSON.stringify(documents)).toBe(original);
});

it("uses deterministic full Unicode case folding across reader locales",()=>{
  const index=indexPublishedDocuments([
    {title:"Straße",folder:null,source:""},
    {title:"Istanbul",folder:null,source:""},
    {title:"Οδυσσεύς",folder:null,source:"Σ"},
  ]);
  expect(searchPublishedDocuments(index,"STRASSE")).toEqual([0]);
  expect(searchPublishedDocuments(index,"istanbul")).toEqual([1]);
  expect(searchPublishedDocuments(index,"σ")).toEqual([2]);
});
