describe("window probe", () => {
  it("reports handles and tries newWindow", async () => {
    const handles = await browser.getWindowHandles();
    console.log("HANDLES", handles);
    try {
      const handle = await browser.newWindow("about:blank");
      console.log("NEW", handle);
      console.log("HANDLES2", await browser.getWindowHandles());
    } catch (error) {
      console.log("NEW_FAILED", String(error));
    }
  });
});
