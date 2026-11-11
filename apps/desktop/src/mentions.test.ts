import { completeMention, mentionQuery, suggestMentions } from "./mentions";

const people = ["octocat", "grace", "linus", "ada-l"].map((login) => ({
  login,
  avatar_url: "",
}));

describe("mentions", () => {
  it("finds the @name before the caret", () => {
    expect(mentionQuery("cc @gr", 6)).toEqual({ start: 3, query: "gr" });
    expect(mentionQuery("@", 1)).toEqual({ start: 0, query: "" });
    expect(mentionQuery("(@ad", 4)).toEqual({ start: 1, query: "ad" });
  });

  it("ignores addresses, code, and finished words", () => {
    expect(mentionQuery("me@example", 10)).toBeNull();
    expect(mentionQuery("`@decorator", 11)).toBeNull();
    expect(mentionQuery("@grace done", 11)).toBeNull();
  });

  it("ranks prefix matches before substring matches", () => {
    expect(suggestMentions(people, "a").map((person) => person.login)).toEqual([
      "ada-l",
      "octocat",
      "grace",
    ]);
    expect(suggestMentions(people, "", 2)).toHaveLength(2);
  });

  it("replaces the typed name and the rest of the word", () => {
    const text = "thanks @gr and";
    const mention = mentionQuery(text, 10)!;
    expect(completeMention(text, 10, mention, "grace")).toEqual({
      text: "thanks @grace and",
      caret: 14,
    });
    const tail = "ping @li";
    expect(completeMention(tail, 8, mentionQuery(tail, 8)!, "linus")).toEqual({
      text: "ping @linus ",
      caret: 12,
    });
  });
});
