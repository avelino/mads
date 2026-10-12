// Runs the generated daily-sync.js against in-memory stubs of the Google Ads script services.
// The Rust test `automation::script` writes the script to a temp file and passes its path in
// DAILY_SYNC_JS.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';

const SCRIPT_PATH = process.env.DAILY_SYNC_JS;
if (!SCRIPT_PATH) {
  throw new Error('set DAILY_SYNC_JS to the path of a generated daily-sync.js');
}
const SCRIPT = readFileSync(SCRIPT_PATH, 'utf8');
const SHEETS_MIME = 'application/vnd.google-apps.spreadsheet';
const XLSX_MIME = 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet';

// ------------------------------------------------------------------ sheets

class Sheet {
  constructor(name, maxRows = 1000, maxCols = 26) {
    this.name = name;
    this.maxRows = maxRows;
    this.maxCols = maxCols;
    this.cells = new Map(); // "r,c" -> value (1-based)
    this.formulas = new Map();
  }
  getName() { return this.name; }
  getMaxRows() { return this.maxRows; }
  getMaxColumns() { return this.maxCols; }
  insertRowsAfter(_after, n) { this.maxRows += n; }
  insertColumnsAfter(_after, n) { this.maxCols += n; }
  getLastRow() {
    let last = 0;
    for (const k of [...this.cells.keys(), ...this.formulas.keys()]) {
      last = Math.max(last, Number(k.split(',')[0]));
    }
    return last;
  }
  getLastColumn() {
    let last = 0;
    for (const k of [...this.cells.keys(), ...this.formulas.keys()]) {
      last = Math.max(last, Number(k.split(',')[1]));
    }
    return last;
  }
  getRange(row, col, rows = 1, cols = 1) {
    if (typeof row === 'string') {
      const m = row.match(/^([A-Z]+)(\d+)$/);
      let c = 0;
      for (const ch of m[1]) c = c * 26 + (ch.charCodeAt(0) - 64);
      return new Range(this, Number(m[2]), c, 1, 1);
    }
    if (row + rows - 1 > this.maxRows || col + cols - 1 > this.maxCols) {
      throw new Error(`out of bounds: ${this.name} ${row},${col} ${rows}x${cols}`);
    }
    return new Range(this, row, col, rows, cols);
  }
  value(row, col) { return this.cells.get(`${row},${col}`) ?? ''; }
  formula(row, col) { return this.formulas.get(`${row},${col}`) ?? ''; }
  // Data rows from row 5, trimmed of empty rows.
  rows(cols) {
    const out = [];
    for (let r = 5; r <= this.getLastRow(); r++) {
      const row = [];
      for (let c = 1; c <= cols; c++) row.push(this.value(r, c));
      if (row.some((v) => v !== '')) out.push(row);
    }
    return out;
  }
}

class Range {
  constructor(sheet, row, col, rows, cols) {
    Object.assign(this, { sheet, row, col, rows, cols });
  }
  key(i, j) { return `${this.row + i},${this.col + j}`; }
  getValues() {
    const out = [];
    for (let i = 0; i < this.rows; i++) {
      const row = [];
      for (let j = 0; j < this.cols; j++) row.push(this.sheet.cells.get(this.key(i, j)) ?? '');
      out.push(row);
    }
    return out;
  }
  setValues(values) {
    assert.equal(values.length, this.rows, 'setValues row count');
    values.forEach((row, i) => {
      assert.equal(row.length, this.cols, 'setValues column count');
      row.forEach((v, j) => this.sheet.cells.set(this.key(i, j), v));
    });
  }
  setValue(v) { this.sheet.cells.set(this.key(0, 0), v); }
  setFormula(f) { this.sheet.formulas.set(this.key(0, 0), f); }
  clearContent() {
    for (let i = 0; i < this.rows; i++) {
      for (let j = 0; j < this.cols; j++) {
        this.sheet.cells.delete(this.key(i, j));
        this.sheet.formulas.delete(this.key(i, j));
      }
    }
  }
}

class Spreadsheet {
  constructor(id, sheets) {
    this.id = id;
    this.sheets = new Map(sheets.map((s) => [s.name, s]));
  }
  getId() { return this.id; }
  getSheetByName(name) { return this.sheets.get(name) ?? null; }
}

// ------------------------------------------------------------------- drive

function iterator(items) {
  let i = 0;
  return { hasNext: () => i < items.length, next: () => items[i++] };
}

class DriveFile {
  constructor(id, name, mime = SHEETS_MIME) { Object.assign(this, { id, name, mime }); }
  getId() { return this.id; }
  getName() { return this.name; }
  getMimeType() { return this.mime; }
}

class Folder {
  constructor(name, files = [], folders = []) { Object.assign(this, { name, files, folders }); }
  getName() { return this.name; }
  getFilesByName(n) { return iterator(this.files.filter((f) => f.name === n)); }
  getFoldersByName(n) { return iterator(this.folders.filter((f) => f.name === n)); }
}

// ------------------------------------------------------------------ ads app

function adsApp(campaigns) {
  const camp = ({ name, groups }) => ({
    getName: () => name,
    adGroups: () => ({
      get: () => iterator(groups.map(([g, enabled]) => ({
        getName: () => g,
        isEnabled: () => enabled,
        isRemoved: () => false,
      }))),
    }),
  });
  return {
    campaigns: () => ({ get: () => iterator(campaigns.map(camp)) }),
    currentAccount: () => ({ getTimeZone: () => 'America/Sao_Paulo' }),
  };
}

// --------------------------------------------------------------- harness

function load({ campaigns = [], folders = [], spreadsheets = [] } = {}) {
  const logs = [];
  const byId = new Map(spreadsheets.map((s) => [s.id, s]));
  const context = {
    AdsApp: adsApp(campaigns),
    SpreadsheetApp: {
      openById(id) {
        const s = byId.get(id);
        if (!s) throw new Error(`no spreadsheet ${id}`);
        return s;
      },
    },
    DriveApp: { getFoldersByName: (n) => iterator(folders.filter((f) => f.name === n)) },
    Logger: { log: (m) => logs.push(String(m)) },
    Utilities: { formatDate: () => '2026-10-10 03:00' },
  };
  vm.createContext(context);
  vm.runInContext(SCRIPT, context);
  return { context, logs };
}

function feedSpreadsheet(id = 'feed-id') {
  return new Spreadsheet(id, [
    new Sheet('Feed', 1000, 38),
    new Sheet('Account Status', 1000, 4),
    new Sheet('Auto Negatives', 1000, 5),
  ]);
}

function feedFolder(name, extra = []) {
  return new Folder(name, [new DriveFile('feed-id', '0. Feed')], extra);
}

// Writes feed data rows from row 5 at the columns the script reads.
function writeFeed(context, sheet, rows) {
  const col = context.FEED_COL;
  sheet.maxRows = Math.max(sheet.maxRows, 4 + rows.length);
  rows.forEach((r, i) => {
    const row = 5 + i;
    sheet.cells.set(`${row},${col.action + 1}`, r.action ?? 'Add');
    sheet.cells.set(`${row},${col.automated + 1}`, r.automated ?? 'Yes');
    sheet.cells.set(`${row},${col.campaign + 1}`, r.campaign);
    sheet.cells.set(`${row},${col.adGroup + 1}`, r.adGroup);
    sheet.cells.set(`${row},${col.keyword + 1}`, r.keyword);
  });
}

function setup({ campaigns = [], automated = ['Catalog - Wines'] } = {}) {
  const feed = feedSpreadsheet();
  const env = load({ campaigns, folders: [feedFolder('Test Folder')], spreadsheets: [feed] });
  env.context.FOLDER_NAME = 'Test Folder';
  env.context.AUTOMATED_CAMPAIGNS = automated;
  return { ...env, feed };
}

// ------------------------------------------------------------------- tests

test('the generated script defines its constants', () => {
  const { context } = load();
  assert.equal(typeof context.FOLDER_NAME, 'string');
  assert.equal(context.FEED_FILE, '0. Feed');
  assert.ok(Array.isArray(context.AUTOMATED_CAMPAIGNS));
  assert.ok(Array.isArray(context.MIRRORS));
  for (const k of ['action', 'automated', 'campaign', 'adGroup', 'keyword']) {
    assert.equal(typeof context.FEED_COL[k], 'number', k);
  }
});

test('Account Status lists enabled and paused groups of automated campaigns only', () => {
  const { context, feed } = setup({
    campaigns: [
      { name: 'Catalog - Wines', groups: [['Alamos Malbec', true], ['Don Melchor', false]] },
      { name: 'Brand', groups: [['Brand terms', true]] },
    ],
  });
  context.main();
  assert.deepEqual(feed.getSheetByName('Account Status').rows(3), [
    ['Catalog - Wines', 'Alamos Malbec', 'Enabled'],
    ['Catalog - Wines', 'Don Melchor', 'Paused'],
  ]);
});

test('Account Status clears rows from the previous run', () => {
  const { context, feed } = setup({
    campaigns: [{ name: 'Catalog - Wines', groups: [['Alamos Malbec', true]] }],
  });
  const status = feed.getSheetByName('Account Status');
  status.cells.set('5,1', 'Catalog - Wines');
  status.cells.set('6,1', 'Catalog - Wines');
  status.cells.set('6,2', 'Gone');
  status.cells.set('6,3', 'Paused');
  context.main();
  assert.deepEqual(status.rows(3), [['Catalog - Wines', 'Alamos Malbec', 'Enabled']]);
});

test('Auto Negatives isolates automated groups by phrase containment', () => {
  const { context, feed } = setup();
  writeFeed(context, feed.getSheetByName('Feed'), [
    { campaign: 'Catalog - Wines', adGroup: 'Vinho Tinto', keyword: 'vinho tinto' },
    { campaign: 'Catalog - Wines', adGroup: 'Vinho Tinto Suave', keyword: 'vinho tinto suave' },
    { campaign: 'Catalog - Wines', adGroup: 'Carménère', keyword: 'carménère' },
    { campaign: 'Catalog - Wines', adGroup: 'Carménère Reserva', keyword: 'carménère reserva' },
    { campaign: 'Catalog - Wines', adGroup: 'Paused Item', keyword: 'vinho', action: '-' },
    { campaign: 'Brand', adGroup: 'Brand terms', keyword: 'vinho tinto barato', automated: 'No' },
    { campaign: 'Brand', adGroup: 'Brand terms', keyword: 'tinto', automated: 'No' },
  ]);
  context.main();
  assert.deepEqual(feed.getSheetByName('Auto Negatives').rows(5), [
    ['Catalog - Wines', 'Carménère', 'carmenere reserva', 'Phrase',
      'Search belongs to ad group Carménère Reserva'],
    ['Catalog - Wines', 'Carménère', 'carménère reserva', 'Phrase',
      'Search belongs to ad group Carménère Reserva'],
    ['Catalog - Wines', 'Vinho Tinto', 'vinho tinto barato', 'Phrase',
      'Search belongs to campaign Brand'],
    ['Catalog - Wines', 'Vinho Tinto', 'vinho tinto suave', 'Phrase',
      'Search belongs to ad group Vinho Tinto Suave'],
  ]);
});

test('main throws past 1000 automated groups and writes nothing', () => {
  const groups = Array.from({ length: 1001 }, (_, i) => [`Group ${i}`, true]);
  const { context, feed } = setup({ campaigns: [{ name: 'Catalog - Wines', groups }] });
  assert.throws(() => context.main(), /1001.*1000/s);
  assert.deepEqual(feed.getSheetByName('Account Status').rows(3), []);
});

test('main throws past 2000 auto negatives', () => {
  const { context, feed } = setup();
  const rows = [{ campaign: 'Catalog - Wines', adGroup: 'Base', keyword: 'wine' }];
  for (let i = 0; i < 2001; i++) {
    rows.push({ campaign: 'Brand', adGroup: `B${i}`, keyword: `wine ${i}`, automated: 'No' });
  }
  writeFeed(context, feed.getSheetByName('Feed'), rows);
  assert.throws(() => context.main(), /2001.*2000/s);
});

test('main fails with a clear message when the folder is missing', () => {
  const { context } = setup();
  context.FOLDER_NAME = 'Nope';
  assert.throws(() => context.main(), /Nope/);
});

test('linkSheets writes IMPORTRANGE into every mirror and keeps going after a failure', () => {
  const d1 = new Spreadsheet('d1-id', [new Sheet('Settings', 20, 3), new Sheet('Feed', 39, 10)]);
  d1.getSheetByName('Feed').cells.set('7,2', 'stale');
  const s1 = new Spreadsheet('s1-id', [new Sheet('Campaigns', 1000, 26)]);
  const feed = feedSpreadsheet();
  const folder = feedFolder('Test Folder', [
    new Folder('Daily', [
      new DriveFile('d1-xlsx', 'D1. New Ad Groups.xlsx', XLSX_MIME),
      new DriveFile('d1-id', 'D1. New Ad Groups'),
    ]),
    new Folder('Structure', [new DriveFile('s1-id', 'S1. New Campaigns.xlsx')]),
  ]);
  const { context, logs } = load({ folders: [folder], spreadsheets: [feed, d1, s1] });
  context.FOLDER_NAME = 'Test Folder';
  context.MIRRORS = [
    ['Daily/D1. New Ad Groups', 'Settings', 'Settings!A5:C11'],
    ['Daily/D1. New Ad Groups', 'Feed', 'Feed!A5:AM156'],
    ['Daily/D2. Pause Ad Groups', 'Feed', 'Feed!A5:AM156'],
    ['Structure/S1. New Campaigns', 'Campaigns', 'Campaigns!A5:N6'],
  ];
  context.linkSheets();

  const d1Feed = d1.getSheetByName('Feed');
  assert.equal(d1Feed.formula(5, 1), '=IMPORTRANGE("feed-id","Feed!A5:AM156")');
  assert.ok(d1Feed.getMaxRows() >= 156);
  assert.ok(d1Feed.getMaxColumns() >= 38);
  assert.equal(d1Feed.value(7, 2), '');
  assert.equal(d1.getSheetByName('Settings').formula(5, 1),
    '=IMPORTRANGE("feed-id","Settings!A5:C11")');
  assert.equal(s1.getSheetByName('Campaigns').formula(5, 1),
    '=IMPORTRANGE("feed-id","Campaigns!A5:N6")');

  assert.ok(logs.some((l) => l.startsWith('linked Daily/D1. New Ad Groups')), logs.join('\n'));
  assert.ok(logs.some((l) => l.startsWith('linked Structure/S1. New Campaigns')), logs.join('\n'));
  assert.ok(logs.some((l) => l.startsWith('FAILED Daily/D2. Pause Ad Groups:')), logs.join('\n'));

  assert.ok(feed.getSheetByName('Account Status').getMaxRows() >= 1004);
  assert.ok(feed.getSheetByName('Auto Negatives').getMaxRows() >= 2004);
});

test('linkSheets is safe to run twice', () => {
  const d1 = new Spreadsheet('d1-id', [new Sheet('Feed', 39, 10)]);
  const folder = feedFolder('Test Folder', [new Folder('Daily', [new DriveFile('d1-id', 'D1. New Ad Groups')])]);
  const { context } = load({ folders: [folder], spreadsheets: [feedSpreadsheet(), d1] });
  context.FOLDER_NAME = 'Test Folder';
  context.MIRRORS = [['Daily/D1. New Ad Groups', 'Feed', 'Feed!A5:AM156']];
  context.linkSheets();
  context.linkSheets();
  assert.equal(d1.getSheetByName('Feed').formula(5, 1), '=IMPORTRANGE("feed-id","Feed!A5:AM156")');
});

test('linkSheets fails loudly when two folders share the name', () => {
  const { context } = load({
    folders: [feedFolder('Twin'), feedFolder('Twin')],
    spreadsheets: [feedSpreadsheet()],
  });
  context.FOLDER_NAME = 'Twin';
  assert.throws(() => context.linkSheets(), /2 folders/);
});
