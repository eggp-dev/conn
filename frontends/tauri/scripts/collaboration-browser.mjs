// One browser connection per test run; context isolation per fixture.
import { chromium } from 'playwright';
export async function collaborationBrowser() {
  const browser = await chromium.launch();
  let context, page;
  const open = async url => {
    await context?.close();
    context = await browser.newContext({ viewport: {width: 1280, height: 900} });
    page = await context.newPage();
    page.setDefaultTimeout(15000);
    await page.goto(url);
  };
  const run = async (action, ...args) => {
    switch (action) {
      case 'open': return open(args[0]);
      case 'close': return browser.close();
      case 'gone': return page.locator(args[0]).waitFor({state: 'detached'});
      case 'wait': return page.locator(args[0]).waitFor({state: 'visible'});
      case 'click': return page.locator(args[0]).click();
      case 'type': return page.locator(args[0]).pressSequentially(args[1]);
      case 'press': return page.keyboard.press(args[0]);
      case 'eval': return page.evaluate(args[0]);
      case 'screenshot': return page.screenshot({path: args[0]});
      case 'set':
        if (args[0] === 'viewport') return page.setViewportSize({width: Number(args[1]), height: Number(args[2])});
        if (args[0] === 'media') return page.emulateMedia({colorScheme: args[1], reducedMotion: 'reduce'});
    }
    throw Error(`Unknown browser action: ${action}`);
  };
  return {run, value: source => page.evaluate(source), open};
}
