import { z } from "zod";

export const extendedComponents = {
  AnsiArt: {
    props: z.object({ title: z.string().max(80), content: z.string().max(16000), height: z.number().int().min(4).max(26) }).strict(),
  },
  Scene: {
    props: z.object({
      title: z.string().max(80),
      rows: z.array(z.string().min(1).max(64).regex(/^[ -~]+$/)).min(1).max(24),
      legend: z.array(z.object({ symbol: z.string().regex(/^[ -~]$/), label: z.string().min(1).max(24), color: z.string().regex(/^#[0-9a-fA-F]{6}$/) }).strict()).min(1).max(12),
    }).strict(),
  },
  BigText: {
    props: z.object({
      text: z.string().min(1).max(24).regex(/^[ -~]+$/),
      font: z.enum(["pixel", "box"]),
    }).strict(),
  },
  PlayingCards: {
    props: z.object({
      title: z.string().max(80),
      cards: z
        .array(
          z.object({
            rank: z.enum(["A","2","3","4","5","6","7","8","9","10","J","Q","K"]),
            suit: z.enum(["clubs","diamonds","hearts","spades"]),
          }).strict(),
        )
        .min(1)
        .max(8),
    }).strict(),
  },
  QRCode: {
    props: z.object({
      title: z.string().max(80),
      data: z.string().min(1).max(120),
    }).strict(),
  },
  Equalizer: {
    props: z.object({
      title: z.string().max(80),
      levels: z.array(z.number().int().min(0).max(100)).min(1).max(32),
    }).strict(),
  },
  BarChart: {
    props: z.object({
      title: z.string().max(80),
      labels: z.array(z.string().max(16)).min(1).max(12),
      values: z.array(z.number().int().min(0).max(1000000000)).min(1).max(12),
    }).strict(),
  },
  Chart: {
    props: z.object({
      title: z.string().max(80),
      points: z
        .array(
          z.array(z.number().min(-1000000).max(1000000)).length(2),
        )
        .min(2)
        .max(64),
      kind: z.enum(["line", "scatter"]),
    }).strict(),
  },
  ScrollView: {
    props: z.object({
      title: z.string().max(80),
      text: z.string().max(12000),
      height: z.number().int().min(4).max(20),
    }).strict(),
  },
  Popup: {
    props: z.object({
      title: z.string().max(80),
      label: z.string().max(80),
      body: z.string().max(2000),
      open: z.boolean(),
    }).strict(),
  },
  Select: {
    props: z.object({
      title: z.string().max(80),
      options: z.array(z.string().min(1).max(80)).min(1).max(100),
      value: z.string().min(1).max(80),
      searchable:z.boolean().default(false),
      bordered:z.boolean().default(true),
    }).strict(),
  },
  MultiSelect: {
    props:z.object({title:z.string().max(80),options:z.array(z.string().min(1).max(80)).min(1).max(100),value:z.array(z.string().min(1).max(80)).max(100),searchable:z.boolean().default(false),bordered:z.boolean().default(true)}).strict(),
  },
  Tabs: {
    props: z.object({
      title: z.string().max(80),
      tabs: z
        .array(
          z.object({
            label: z.string().min(1).max(40),
            text: z.string().max(2000),
          }).strict(),
        )
        .min(1)
        .max(8),
      value: z.string().min(1).max(40),
    }).strict(),
  },
  Slider: {
    props: z.object({
      label: z.string().max(80),
      value: z.number().min(0).max(100),
      min: z.number().min(0).max(100),
      max: z.number().min(0).max(100),
      step: z.number().min(0.1).max(100),
    }).strict(),
  },
};

export const extendedInstructions = [
  "MultiSelect: bordered (boolean, default true; false omits frame/title), title, options (1-100 unique nonempty strings<=80), value {$bindState:'/key'} bound to an array of selected option strings, searchable boolean. Initialize with [] or exact options. Space toggles a choice; typing filters locally when searchable. Use for inventory/loadout/features; do not simulate it with text.",
  'AnsiArt: title, content (string<=16000 with newlines and ANSI SGR colors), height (minimum rows, int 4-26; native measurement grows to fit decoded lines plus borders). For colored text illustrations, ships, portraits, banners. Use JSON \\u001b for ESC, 16/256/RGB colors; cursor movement is not supported.',
  'Scene: title, rows (1-24 printable ASCII strings, each 1-64 chars), legend (1-12 {symbol:one ASCII character,label:string<=24,color:"#RRGGBB"}). Legend symbols must be unique. List important entities and terrain; unlisted decorative characters use muted gray. Native code centers and colors this tile scene, showing its legend. For GAME interfaces always include a prominent Scene or AnsiArt near the top: a dungeon with rooms, passages, player and enemies; a sector map with stars, planets, ships and routes; or a domain-specific illustration. Prefer Scene for maps: about 8-12 rows of 24-40 columns. Make spatially meaningful artwork, not a table of coordinates. Include the visual even when other controls need to be omitted. Never substitute plain Text for the game visual.',
  "BigText: text (string 1-24 printable ASCII), font (pixel|box).",
  "PlayingCards: title (string<=80), cards (array 1-8 of {rank, suit}).",
  "QRCode: title (string<=80), data (string 1-120).",
  "Equalizer: title (string<=80), levels (array 1-32 integers 0-100).",
  "BarChart: title (string<=80), labels (array 1-12 string<=16), values (array 1-12 integers 0-1000000000); labels.length must equal values.length.",
  "Chart: title (string<=80), points (array 2-64 tuples [number,number] each -1000000..1000000), kind (line|scatter).",
  "ScrollView: title (string<=80), text (string<=12000), height (int 4-20).",
  "Popup: title (string<=80), label (string<=80), body (string<=2000), open (boolean). Popup.open binds existing boolean via $bindState, initialize false. It INCLUDES its own trigger button; NEVER add a separate Button to open or close it. Enter or Esc dismisses it.",
  "Select: bordered (boolean, default true; false omits frame/title), title (string<=80), options (array 1-100 unique string 1-80), value (string 1-80). Select.value binds existing string matching one option label. searchable:true enables local fuzzy filtering and Enter commits the highlighted result.",
  "Tabs: title (string<=80), tabs (array 1-8 of {label string 1-40, text<=2000}), value (string 1-40). Tabs.value binds existing string matching one tab label.",
  "Slider: label (string<=80), value (number 0-100), min (number 0-100), max (number 0-100), step (number 0.1-100). Slider.value binds existing number inside min/max. min must be less than max.",
  "These widgets add no custom events. Popup, Select, Tabs and Slider handle interaction natively. Other props are literal. Use exact rank A,2,3,4,5,6,7,8,9,10,J,Q,K and suit clubs,diamonds,hearts,spades for cards. Tabs contain text pages with unique labels. Use ScrollView for long logs, Select for one choice, Slider for adjustable numbers, Chart for XY plots, BarChart for labeled comparisons, Equalizer for frequency levels, and BigText for short readouts.",
].join(" ");

export const extendedSamples = [
  { type: 'AnsiArt', props: { title: 'Beacon', content: '\u001b[38;2;70;210;255m   /\\\n--<  >--\n   \\/\u001b[0m', height: 6 } },
  { type: 'Scene', props: { title: 'Dungeon', rows: ['########', '#@..g..#', '###..###'], legend: [{symbol:'#',label:'Wall',color:'#667788'},{symbol:'@',label:'You',color:'#ffb000'},{symbol:'.',label:'Floor',color:'#334455'},{symbol:'g',label:'Goblin',color:'#ff5544'}] } },
  {
    type: "BigText",
    props: { text: "HELLO", font: "pixel" },
  },
  {
    type: "PlayingCards",
    props: {
      title: "Hand",
      cards: [
        { rank: "A", suit: "spades" },
        { rank: "10", suit: "hearts" },
      ],
    },
  },
  {
    type: "QRCode",
    props: { title: "Link", data: "https://example.com" },
  },
  {
    type: "Equalizer",
    props: { title: "EQ", levels: [80, 55, 30] },
  },
  {
    type: "BarChart",
    props: {
      title: "Sales",
      labels: ["Q1", "Q2"],
      values: [120, 200],
    },
  },
  {
    type: "Chart",
    props: {
      title: "Trend",
      points: [
        [0, 0],
        [1, 5],
        [2, 3],
      ],
      kind: "line",
    },
  },
  {
    type: "ScrollView",
    props: { title: "Info", text: "Some scrollable content.", height: 8 },
  },
  {
    type: "Popup",
    props: { title: "Note", label: "OK", body: "Done.", open: false },
  },
  {
    type: "Select",
    props: { title: "Lang", options: ["en", "fr", "de"], value: "en" },
  },
  {
    type: "MultiSelect",
    props: { title: "Features", options: ["Search", "Export", "History"], value: ["Search"], searchable: true },
  },
  {
    type: "Tabs",
    props: {
      title: "Panels",
      tabs: [
        { label: "A", text: "First panel." },
        { label: "B", text: "Second panel." },
      ],
      value: "A",
    },
  },
  {
    type: "Slider",
    props: { label: "Vol", value: 50, min: 0, max: 100, step: 1 },
  },
];
