// Runs setupImageCampaigns() of the generated daily-sync.js against in-memory stubs of the
// Google Ads script services. The Rust test `automation::script` writes the script to a temp
// file and passes its path in DAILY_SYNC_JS.

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
const CID = '1234567890';

// Every top-level key a MutateOperation may carry in this script.
const VALID_KEYS = new Set([
  'campaignBudgetOperation', 'campaignOperation', 'assetOperation', 'assetGroupOperation',
  'assetGroupAssetOperation', 'adGroupOperation', 'adGroupAdOperation',
  'campaignAssetOperation', 'campaignCriterionOperation',
  // Demand Gen targets language and location on its ad groups, Performance Max takes search
  // themes as asset group signals: both are MutateOperation fields too.
  'adGroupCriterionOperation', 'assetGroupSignalOperation',
]);

// ------------------------------------------------------------------- stubs

// Objects built inside the vm context have its prototypes: compare plain copies.
const plain = (v) => JSON.parse(JSON.stringify(v));

function iterator(items) {
  let i = 0;
  return { hasNext: () => i < items.length, next: () => items[i++] };
}

/** A tab whose data rows start at row 5, read through getRange(...).getValues(). */
function tab(rows, width) {
  return {
    getLastRow: () => 4 + rows.length,
    getRange: (row, col, n, w) => ({
      getValues: () => {
        assert.equal(row, 5);
        assert.equal(col, 1);
        assert.ok(w >= width, `read ${w} columns, need ${width}`);
        return rows.slice(0, n).map((r) => {
          const out = r.slice();
          while (out.length < w) out.push('');
          return out;
        });
      },
    }),
  };
}

function picture(name) {
  return {
    getName: () => name,
    getBlob: () => ({ getBytes: () => [...Buffer.from(name)] }),
  };
}

function folder(name, files = [], folders = []) {
  return {
    name,
    getName: () => name,
    getFilesByName: (n) => iterator(files.filter((f) => f.name === n).map((f) => f.file)),
    getFoldersByName: (n) => iterator(folders.filter((f) => f.name === n)),
  };
}

function sheetFile(id, name) {
  return { name, file: { getId: () => id, getName: () => name, getMimeType: () => SHEETS_MIME } };
}

function imageFile(name) {
  return { name, file: picture(name) };
}

/** `live`: rows the campaign search returns. `failMutate`: names whose mutateAll fails. */
function adsApp({ live = [], geos = {}, failMutate = [] }) {
  const calls = { mutateAll: [], mutate: [], searches: [] };
  const ok = (name) => ({ isSuccessful: () => true, getResourceName: () => name, getErrorMessages: () => [] });
  const bad = (msg) => ({ isSuccessful: () => false, getResourceName: () => '', getErrorMessages: () => [msg] });
  const app = {
    calls,
    currentAccount: () => ({ getCustomerId: () => '123-456-7890', getTimeZone: () => 'UTC' }),
    search(query) {
      calls.searches.push(query);
      if (/FROM campaign\b/.test(query)) {
        return iterator(live.map((c) => ({
          campaign: { id: c.id, name: c.name, status: c.status ?? 'ENABLED',
                      resourceName: `customers/${CID}/campaigns/${c.id}` },
          campaignBudget: { resourceName: `customers/${CID}/campaignBudgets/${c.id}9`,
                            amountMicros: String(c.micros) },
        })));
      }
      if (/FROM language_constant/.test(query)) {
        const m = query.match(/code = '([^']*)'/);
        return iterator(m && m[1] === 'pt'
          ? [{ languageConstant: { resourceName: 'languageConstants/1014' } }] : []);
      }
      if (/FROM geo_target_constant/.test(query)) {
        const m = query.match(/(?:name|canonical_name) = '([^']*)'/);
        const id = m ? geos[m[1]] : undefined;
        return iterator(id ? [{ geoTargetConstant: { resourceName: `geoTargetConstants/${id}`,
                                                     targetType: 'Country' } }] : []);
      }
      throw new Error(`unexpected query ${query}`);
    },
    mutateAll(ops, options) {
      calls.mutateAll.push(plain({ ops, options }));
      const name = ops.find((o) => o.campaignOperation)?.campaignOperation.create.name;
      if (failMutate.includes(name)) return ops.map(() => bad('RESOURCE_LIMIT'));
      return ops.map((_, i) => ok(`r${i}`));
    },
    mutate(op) {
      calls.mutate.push(plain(op));
      return ok('updated');
    },
  };
  return app;
}

// ------------------------------------------------------------------- feed

function campaignRow(context, r) {
  const c = context.IMAGE_CAMPAIGN_COL;
  const row = [];
  const set = (k, v) => { row[c[k]] = v ?? ''; };
  set('key', r.key);
  set('campaign', r.name);
  set('campaignType', r.type ?? 'Performance Max');
  set('budget', r.budget ?? '40.00');
  set('bidStrategy', r.bid ?? 'maximize_conversions');
  set('maxCpc', r.maxCpc);
  set('language', r.language ?? 'pt');
  set('location', r.location ?? 'Brazil');
  set('finalUrl', r.finalUrl ?? 'https://vinellu.com');
  set('businessName', r.businessName ?? 'Vinellu');
  set('appId', r.appId);
  set('appStore', r.appStore);
  set('action', r.action ?? 'Add');
  for (let i = 0; i < row.length; i++) if (row[i] === undefined) row[i] = '';
  return row;
}

function assetRow(context, [key, group, field, text, file]) {
  const c = context.IMAGE_ASSET_COL;
  const row = [];
  row[c.key] = key;
  row[c.group] = group;
  row[c.field] = field;
  row[c.text] = text ?? '';
  row[c.imageFile] = file ?? '';
  for (let i = 0; i < row.length; i++) if (row[i] === undefined) row[i] = '';
  return row;
}

function pmaxAssets(key, group = 'Wine lovers') {
  return [
    [key, group, 'HEADLINE', 'Wine Ratings'],
    [key, group, 'HEADLINE', 'Scan the Label'],
    [key, group, 'HEADLINE', 'Real Reviews'],
    [key, group, 'LONG_HEADLINE', 'See the rating before you open the bottle'],
    [key, group, 'DESCRIPTION', 'Real ratings from people who drank it.'],
    [key, group, 'DESCRIPTION', 'Scan any label and see the score.'],
    [key, group, 'SEARCH_THEME', 'wine ratings'],
    [key, group, 'BUSINESS_NAME', 'Vinellu'],
    [key, group, 'MARKETING_IMAGE', '', `${key}/landscape.jpg`],
    [key, group, 'SQUARE_MARKETING_IMAGE', '', `${key}/square.jpg`],
    [key, group, 'LOGO', '', 'logo.png'],
  ];
}

/**
 * Loads the script with a feed holding `campaigns` and `assets`, and an Images folder holding
 * `pictures` (paths relative to Images/).
 */
function setup({ campaigns, assets, pictures, live = [], geos = { Brazil: '2076' },
                 failMutate = [] }) {
  const logs = [];
  const ads = adsApp({ live, geos, failMutate });
  const tabs = {};
  const feed = { getId: () => 'feed-id', getSheetByName: (n) => tabs[n] ?? null };
  const byDir = {};
  for (const p of pictures) {
    const parts = p.split('/');
    const dir = parts.length > 1 ? parts[0] : '';
    (byDir[dir] ??= []).push(imageFile(parts[parts.length - 1]));
  }
  const sub = Object.keys(byDir).filter((d) => d !== '').map((d) => folder(d, byDir[d]));
  const images = folder('Images', byDir[''] ?? [], sub);
  const root = folder('Test Folder', [sheetFile('feed-id', '0. Feed')], [images]);
  const context = {
    AdsApp: ads,
    SpreadsheetApp: { openById: (id) => { assert.equal(id, 'feed-id'); return feed; } },
    DriveApp: { getFoldersByName: (n) => iterator(n === 'Test Folder' ? [root] : []) },
    Logger: { log: (m) => logs.push(String(m)) },
    Utilities: {
      formatDate: () => '2026-10-10 03:00',
      base64Encode: (bytes) => Buffer.from(bytes).toString('base64'),
    },
  };
  vm.createContext(context);
  vm.runInContext(SCRIPT, context);
  context.FOLDER_NAME = 'Test Folder';
  tabs['Image Campaigns'] = tab(campaigns.map((r) => campaignRow(context, r)), 13);
  tabs['Image Assets'] = tab(assets.map((r) => assetRow(context, r)), 5);
  return { context, logs, calls: ads.calls };
}

// ---------------------------------------------------------------- helpers

function key(op) {
  const keys = Object.keys(op);
  assert.equal(keys.length, 1, JSON.stringify(op));
  return keys[0];
}

function creates(ops, k) {
  return ops.filter((o) => key(o) === k).map((o) => o[k].create);
}

/** Every resource name an operation refers to (fields ending in a resource path). */
function references(value, out = []) {
  if (typeof value === 'string') {
    if (value.startsWith(`customers/${CID}/`)) out.push(value);
  } else if (Array.isArray(value)) {
    value.forEach((v) => references(v, out));
  } else if (value && typeof value === 'object') {
    for (const [k, v] of Object.entries(value)) if (k !== 'resourceName') references(v, out);
  }
  return out;
}

function assertTempIdsConsistent(ops) {
  const created = new Set();
  for (const op of ops) {
    const k = key(op);
    assert.ok(VALID_KEYS.has(k), `invalid MutateOperation key ${k}`);
    const body = op[k].create;
    assert.ok(body, `${k} is not a create`);
    for (const ref of references(body)) {
      assert.ok(created.has(ref), `${k} refers to ${ref} before it is created`);
    }
    if (body.resourceName) {
      const id = body.resourceName.split('/').pop();
      assert.ok(Number(id) < 0, `temporary id ${body.resourceName} is not negative`);
      assert.ok(!created.has(body.resourceName), `${body.resourceName} created twice`);
      created.add(body.resourceName);
    }
  }
}

const PMAX = { key: 'pmax-wines', name: 'PMax - Wines' };
const PMAX_PICTURES = ['pmax-wines/landscape.jpg', 'pmax-wines/square.jpg', 'logo.png'];

// ------------------------------------------------------------------- tests

test('the generated script injects the image tab columns', () => {
  const { context } = setup({ campaigns: [], assets: [], pictures: [] });
  for (const k of ['key', 'campaign', 'campaignType', 'budget', 'bidStrategy', 'maxCpc',
                   'language', 'location', 'finalUrl', 'businessName', 'appId', 'appStore',
                   'action']) {
    assert.equal(typeof context.IMAGE_CAMPAIGN_COL[k], 'number', k);
  }
  for (const k of ['key', 'group', 'field', 'text', 'imageFile']) {
    assert.equal(typeof context.IMAGE_ASSET_COL[k], 'number', k);
  }
});

test('a new Performance Max campaign is one atomic mutateAll, paused, with consistent temp ids', () => {
  const { context, logs, calls } = setup({
    campaigns: [PMAX], assets: pmaxAssets(PMAX.key), pictures: PMAX_PICTURES,
  });
  context.setupImageCampaigns();

  assert.equal(calls.mutateAll.length, 1);
  const { ops, options } = calls.mutateAll[0];
  assert.deepEqual(options, { partialFailure: false });
  assertTempIdsConsistent(ops);

  const [budget] = creates(ops, 'campaignBudgetOperation');
  assert.equal(budget.amountMicros, '40000000');
  assert.equal(budget.explicitlyShared, false);
  assert.equal(budget.deliveryMethod, 'STANDARD');

  const [campaign] = creates(ops, 'campaignOperation');
  assert.equal(campaign.name, 'PMax - Wines');
  assert.equal(campaign.status, 'PAUSED');
  assert.equal(campaign.advertisingChannelType, 'PERFORMANCE_MAX');
  assert.equal(campaign.advertisingChannelSubType, undefined);
  assert.equal(campaign.campaignBudget, budget.resourceName);
  assert.deepEqual(campaign.maximizeConversions, {});
  assert.equal(campaign.containsEuPoliticalAdvertising, 'DOES_NOT_CONTAIN_EU_POLITICAL_ADVERTISING');
  assert.equal(campaign.brandGuidelinesEnabled, true);

  const [group] = creates(ops, 'assetGroupOperation');
  assert.equal(group.name, 'Wine lovers');
  assert.equal(group.campaign, campaign.resourceName);
  assert.deepEqual(group.finalUrls, ['https://vinellu.com']);

  const links = creates(ops, 'assetGroupAssetOperation');
  const fields = links.map((l) => l.fieldType).sort();
  assert.deepEqual(fields, ['DESCRIPTION', 'DESCRIPTION', 'HEADLINE', 'HEADLINE', 'HEADLINE',
    'LONG_HEADLINE', 'MARKETING_IMAGE', 'SQUARE_MARKETING_IMAGE']);
  links.forEach((l) => assert.equal(l.assetGroup, group.resourceName));

  // brand guidelines: business name and logo are linked to the campaign
  const brand = creates(ops, 'campaignAssetOperation').map((a) => a.fieldType).sort();
  assert.deepEqual(brand, ['BUSINESS_NAME', 'LOGO']);

  const [signal] = creates(ops, 'assetGroupSignalOperation');
  assert.deepEqual(signal.searchTheme, { text: 'wine ratings' });

  const images = creates(ops, 'assetOperation').filter((a) => a.imageAsset);
  assert.equal(images.length, 3);
  images.forEach((a) => {
    assert.equal(a.type, 'IMAGE');
    assert.ok(a.imageAsset.data.length > 0);
  });

  const criteria = creates(ops, 'campaignCriterionOperation');
  assert.deepEqual(criteria.map((c) => c.language?.languageConstant ?? c.location?.geoTargetConstant),
    ['languageConstants/1014', 'geoTargetConstants/2076']);
  criteria.forEach((c) => assert.equal(c.campaign, campaign.resourceName));

  assert.ok(logs.includes('created PMax - Wines'), logs.join('\n'));
});

test('a new Demand Gen campaign carries a multi-asset ad and ad group targeting', () => {
  const key = 'dg-wines';
  const { context, logs, calls } = setup({
    campaigns: [{ key, name: 'DG - Wines', type: 'Demand Gen', bid: 'maximize_clicks',
                  maxCpc: '1.50' }],
    assets: [
      [key, 'Wine fans', 'HEADLINE', 'Wine Ratings'],
      [key, 'Wine fans', 'DESCRIPTION', 'Real ratings.'],
      [key, 'Wine fans', 'LONG_HEADLINE', 'See the rating before you open it'],
      [key, 'Wine fans', 'BUSINESS_NAME', 'Vinellu'],
      [key, 'Wine fans', 'MARKETING_IMAGE', '', `${key}/a.jpg`],
      [key, 'Wine fans', 'SQUARE_MARKETING_IMAGE', '', `${key}/b.jpg`],
      [key, 'Wine fans', 'PORTRAIT_MARKETING_IMAGE', '', `${key}/c.jpg`],
      [key, 'Wine fans', 'LOGO', '', 'logo.png'],
    ],
    pictures: [`${key}/a.jpg`, `${key}/b.jpg`, `${key}/c.jpg`, 'logo.png'],
  });
  context.setupImageCampaigns();

  assert.equal(calls.mutateAll.length, 1);
  const { ops, options } = calls.mutateAll[0];
  assert.deepEqual(options, { partialFailure: false });
  assertTempIdsConsistent(ops);

  const [campaign] = creates(ops, 'campaignOperation');
  assert.equal(campaign.advertisingChannelType, 'DEMAND_GEN');
  assert.equal(campaign.status, 'PAUSED');
  assert.deepEqual(campaign.targetSpend, {});

  const [group] = creates(ops, 'adGroupOperation');
  assert.equal(group.campaign, campaign.resourceName);
  assert.equal(group.type, undefined);

  const [ad] = creates(ops, 'adGroupAdOperation');
  assert.equal(ad.adGroup, group.resourceName);
  const info = ad.ad.demandGenMultiAssetAd;
  assert.ok(info, 'demandGenMultiAssetAd');
  assert.deepEqual(info.headlines, [{ text: 'Wine Ratings' }]);
  assert.deepEqual(info.descriptions, [{ text: 'Real ratings.' }]);
  assert.equal(info.businessName, 'Vinellu');
  assert.equal(info.marketingImages.length, 1);
  assert.equal(info.squareMarketingImages.length, 1);
  assert.equal(info.portraitMarketingImages.length, 1);
  assert.equal(info.logoImages.length, 1);
  assert.deepEqual(ad.ad.finalUrls, ['https://vinellu.com']);

  assert.equal(creates(ops, 'campaignCriterionOperation').length, 0);
  const targeting = creates(ops, 'adGroupCriterionOperation');
  assert.equal(targeting.length, 2);
  targeting.forEach((c) => assert.equal(c.adGroup, group.resourceName));

  assert.ok(logs.some((l) => /DG - Wines.*Max CPC/.test(l)), logs.join('\n'));
  assert.ok(logs.includes('created DG - Wines'), logs.join('\n'));
});

test('a new App campaign carries appCampaignSetting and an app ad', () => {
  const key = 'app-installs';
  const { context, logs, calls } = setup({
    campaigns: [{ key, name: 'App - Installs', type: 'App', bid: 'manual_cpc',
                  appId: 'com.vinellu.app', appStore: 'Google Play', businessName: '' }],
    assets: [
      [key, 'Installs', 'HEADLINE', 'Rate any wine'],
      [key, 'Installs', 'DESCRIPTION', 'Scan the label.'],
      [key, 'Installs', 'MARKETING_IMAGE', '', `${key}/a.jpg`],
    ],
    pictures: [`${key}/a.jpg`],
  });
  context.setupImageCampaigns();

  const { ops } = calls.mutateAll[0];
  assertTempIdsConsistent(ops);
  const [campaign] = creates(ops, 'campaignOperation');
  assert.equal(campaign.advertisingChannelType, 'MULTI_CHANNEL');
  assert.equal(campaign.advertisingChannelSubType, 'APP_CAMPAIGN');
  assert.deepEqual(campaign.appCampaignSetting, {
    appId: 'com.vinellu.app',
    appStore: 'GOOGLE_APP_STORE',
    biddingStrategyGoalType: 'OPTIMIZE_INSTALLS_WITHOUT_TARGET_INSTALL_COST',
  });
  assert.deepEqual(campaign.maximizeConversions, {});

  const [ad] = creates(ops, 'adGroupAdOperation');
  assert.deepEqual(ad.ad.appAd.headlines, [{ text: 'Rate any wine' }]);
  assert.deepEqual(ad.ad.appAd.descriptions, [{ text: 'Scan the label.' }]);
  assert.equal(ad.ad.appAd.images.length, 1);
  assert.ok(logs.some((l) => /App - Installs.*manual_cpc/.test(l)), logs.join('\n'));
});

test('an Apple app maps to APPLE_APP_STORE', () => {
  const key = 'app-ios';
  const { context, calls } = setup({
    campaigns: [{ key, name: 'App - iOS', type: 'App', appId: '123456',
                  appStore: 'Apple App Store' }],
    assets: [[key, 'Installs', 'HEADLINE', 'Rate any wine'],
             [key, 'Installs', 'DESCRIPTION', 'Scan the label.']],
    pictures: [],
  });
  context.setupImageCampaigns();
  const [campaign] = creates(calls.mutateAll[0].ops, 'campaignOperation');
  assert.equal(campaign.appCampaignSetting.appStore, 'APPLE_APP_STORE');
});

test('an existing campaign gets one budget update and no create', () => {
  const { context, logs, calls } = setup({
    campaigns: [{ ...PMAX, action: 'Edit', budget: '50.00' }],
    assets: pmaxAssets(PMAX.key), pictures: PMAX_PICTURES,
    live: [{ id: 77, name: 'PMax - Wines', micros: 40000000 }],
  });
  context.setupImageCampaigns();
  assert.equal(calls.mutateAll.length, 0);
  assert.equal(calls.mutate.length, 1);
  assert.deepEqual(calls.mutate[0], {
    campaignBudgetOperation: {
      update: { resourceName: `customers/${CID}/campaignBudgets/779`, amountMicros: '50000000' },
      updateMask: 'amount_micros',
    },
  });
  assert.ok(logs.includes('budget updated PMax - Wines'), logs.join('\n'));
});

test('an existing campaign with the same budget is unchanged', () => {
  const { context, logs, calls } = setup({
    campaigns: [PMAX], assets: pmaxAssets(PMAX.key), pictures: PMAX_PICTURES,
    live: [{ id: 77, name: 'PMax - Wines', micros: 40000000 }],
  });
  context.setupImageCampaigns();
  assert.equal(calls.mutateAll.length + calls.mutate.length, 0);
  assert.ok(logs.includes('unchanged PMax - Wines'), logs.join('\n'));
});

test('Pause pauses an enabled campaign and leaves a paused one alone', () => {
  const { context, logs, calls } = setup({
    campaigns: [
      { key: 'old', name: 'Old PMax', action: 'Pause', budget: '' },
      { key: 'older', name: 'Older PMax', action: 'Pause', budget: '' },
      { key: 'gone', name: 'Gone PMax', action: 'Pause', budget: '' },
    ],
    assets: [], pictures: [],
    live: [{ id: 5, name: 'Old PMax', micros: 1 },
           { id: 6, name: 'Older PMax', micros: 1, status: 'PAUSED' }],
  });
  context.setupImageCampaigns();
  assert.equal(calls.mutateAll.length, 0);
  assert.deepEqual(calls.mutate, [{
    campaignOperation: {
      update: { resourceName: `customers/${CID}/campaigns/5`, status: 'PAUSED' },
      updateMask: 'status',
    },
  }]);
  assert.ok(logs.includes('paused Old PMax'), logs.join('\n'));
  assert.ok(logs.includes('unchanged Older PMax'), logs.join('\n'));
  assert.ok(logs.includes('unchanged Gone PMax'), logs.join('\n'));
});

test('a missing picture fails only its campaign and names the file', () => {
  const other = { key: 'pmax-beer', name: 'PMax - Beer' };
  const { context, logs, calls } = setup({
    campaigns: [PMAX, other],
    assets: [...pmaxAssets(PMAX.key), ...pmaxAssets(other.key)],
    pictures: ['pmax-wines/landscape.jpg', 'logo.png', 'pmax-beer/landscape.jpg',
               'pmax-beer/square.jpg'],
  });
  context.setupImageCampaigns();
  const failed = logs.find((l) => l.startsWith('FAILED PMax - Wines:'));
  assert.ok(failed, logs.join('\n'));
  assert.match(failed, /pmax-wines\/square\.jpg/);
  assert.equal(calls.mutateAll.length, 1);
  const [campaign] = creates(calls.mutateAll[0].ops, 'campaignOperation');
  assert.equal(campaign.name, 'PMax - Beer');
  assert.ok(logs.includes('created PMax - Beer'), logs.join('\n'));
});

test('an API failure is logged and the next campaign is still created', () => {
  const other = { key: 'pmax-beer', name: 'PMax - Beer' };
  const { context, logs, calls } = setup({
    campaigns: [PMAX, other],
    assets: [...pmaxAssets(PMAX.key), ...pmaxAssets(other.key)],
    pictures: [...PMAX_PICTURES, 'pmax-beer/landscape.jpg', 'pmax-beer/square.jpg'],
    failMutate: ['PMax - Wines'],
  });
  context.setupImageCampaigns();
  assert.equal(calls.mutateAll.length, 2);
  assert.ok(logs.includes('FAILED PMax - Wines: RESOURCE_LIMIT'), logs.join('\n'));
  assert.ok(logs.includes('created PMax - Beer'), logs.join('\n'));
});

test('an unknown location is skipped with a warning, not a failure', () => {
  const { context, logs, calls } = setup({
    campaigns: [{ ...PMAX, location: 'Atlantis' }],
    assets: pmaxAssets(PMAX.key), pictures: PMAX_PICTURES,
  });
  context.setupImageCampaigns();
  const criteria = creates(calls.mutateAll[0].ops, 'campaignCriterionOperation');
  assert.equal(criteria.length, 1);
  assert.ok(criteria[0].language);
  assert.ok(logs.some((l) => l.startsWith('WARNING PMax - Wines:') && /Atlantis/.test(l)),
    logs.join('\n'));
  assert.ok(logs.includes('created PMax - Wines'), logs.join('\n'));
});

test('manual_cpc on Performance Max maps to maximize conversions and says so', () => {
  const { context, logs, calls } = setup({
    campaigns: [{ ...PMAX, bid: 'manual_cpc' }],
    assets: pmaxAssets(PMAX.key), pictures: PMAX_PICTURES,
  });
  context.setupImageCampaigns();
  const [campaign] = creates(calls.mutateAll[0].ops, 'campaignOperation');
  assert.deepEqual(campaign.maximizeConversions, {});
  assert.equal(campaign.manualCpc, undefined);
  assert.ok(logs.some((l) => /PMax - Wines.*manual_cpc.*maximize conversions/.test(l)),
    logs.join('\n'));
});

test('the campaign search uses the documented query', () => {
  const { context, calls } = setup({ campaigns: [], assets: [], pictures: [] });
  context.setupImageCampaigns();
  assert.ok(calls.searches.some((q) => /FROM campaign WHERE campaign\.status != REMOVED/.test(q)
    && /campaign_budget\.amount_micros/.test(q)), calls.searches.join('\n'));
});
