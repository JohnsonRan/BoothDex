import type { LegalTexts } from "./legal";

const texts: LegalTexts = {
  noContact: "[contact address not set]",
  privacy: {
    title: "Privacy Policy",
    summary:
      "This page explains what personal data this BoothDex site handles, why, and for how long. The site shows no ads, uses no analytics or tracking, and doesn't sell your data.",
    sections: [
      {
        id: "operator",
        heading: "Who runs this site",
        body: [
          "This site is run by {operator}, who is responsible for the personal data it handles. For any question or request about your data, contact {contact}.",
          "BoothDex is open-source software, published at {source}. This policy describes how it works on this site.",
        ],
      },
      {
        id: "data",
        heading: "What we collect",
        body: [
          "When you use the site, whether or not you're signed in:",
          {
            list: [
              "Your IP address and browser details, which come with every request. They're used to deliver pages and to limit abusive traffic; the rate limits are kept in memory only.",
              "What you search for. When semantic search is on, the search text is sent to an AI provider to work out its meaning, with nothing that identifies you.",
            ],
          },
          "When you create an account:",
          {
            list: [
              "Your username, display name and email address.",
              "Your password, stored only as an Argon2 hash that can't be turned back into the password.",
              "Your settings (such as language, currency, appearance and avatar), favourites, saved searches and recently viewed items (the latest 500).",
              "For each device you sign in on: when you signed in, the browser (user agent) and the IP address, so you can tell your devices apart in Settings. The time of your last sign-in is also recorded.",
            ],
          },
        ],
      },
      {
        id: "cookies",
        heading: "Cookies and local storage",
        body: [
          "The site uses only the cookies and browser storage below. Each one is needed for the site to work or keeps a choice you made, so the site doesn't ask for consent. None of them track you across other sites.",
          {
            table: {
              columns: ["Name", "Type", "Purpose", "Kept for"],
              rows: [
                [
                  "session",
                  "Cookie",
                  "Keeps you signed in. Scripts can't read it.",
                  "30 days, or until you sign out",
                ],
                [
                  "settings",
                  "Cookie",
                  "Your language, currency and appearance, so pages arrive in them.",
                  "1 year",
                ],
                [
                  "searchHistory",
                  "Local storage",
                  "Your recent searches, for suggestions.",
                  "Until you clear it",
                ],
                [
                  "viewedItems",
                  "Local storage",
                  "Items you've opened. When you sign in, they move to your account.",
                  "Until you clear it",
                ],
                [
                  "myAvatar",
                  "Local storage",
                  "The avatar you picked last.",
                  "Until you clear it",
                ],
                [
                  "account",
                  "Local storage",
                  "Whether this browser is signed in, so the page looks right while it loads.",
                  "Until you sign out",
                ],
              ],
            },
          },
          "If the site is served through a network provider such as Cloudflare, that provider may set its own cookies to protect the site from attacks.",
          "You can delete all of these at any time in your browser's settings for this site.",
        ],
      },
      {
        id: "use",
        heading: "How we use it",
        body: [
          {
            list: [
              "To run the site and your account: search, favourites, saved searches and your settings on every device.",
              "To keep the site and your account secure, for example by limiting abuse and letting you sign out devices.",
              "To help you and to stop abuse: site administrators can see account details and signed-in devices when needed.",
            ],
          },
          "We don't use your data for advertising or profiling, and we never sell it. Where the GDPR applies, we rely on providing the service you signed up for and on our legitimate interest in keeping the site secure.",
        ],
      },
      {
        id: "sharing",
        heading: "Who else receives it",
        body: [
          {
            list: [
              "Hosting and network providers that run the servers and connections for this site.",
              "The AI provider used for semantic search and item translation. It receives search text and public item information, never your account details.",
              "Authorities, when the law requires it.",
            ],
          },
          "Item images are loaded through this site's own server, so BOOTH doesn't see what you browse here. Once you go to an item on BOOTH, BOOTH's privacy policy applies.",
          "These providers may process data in countries other than yours.",
        ],
      },
      {
        id: "retention",
        heading: "How long we keep it",
        body: [
          {
            list: [
              "Account data: until you delete your account.",
              "Signed-in devices: until you sign out or the session ends after 30 days. Ended sessions are deleted within an hour.",
              "Recently viewed items: the latest 500.",
              "Search text cached for semantic search: deleted after 90 days without use. It isn't linked to anyone.",
              "Server logs: overwritten once they reach a small size limit.",
            ],
          },
        ],
      },
      {
        id: "rights",
        heading: "Your choices and rights",
        body: [
          "You can change your settings at any time, and sign out other devices in Settings.",
          "You can delete your account at any time in Settings > Your data. This immediately removes its devices, favourites, saved searches and viewing history.",
          "To get a copy of your data or correct it, or if you can't sign in to your account, contact {contact}.",
          "Depending on where you live (for example under the GDPR in the EU and UK, the APPI in Japan, PIPA in Korea or PIPL in China), you may have further rights, such as objecting to processing or complaining to a data protection authority.",
        ],
      },
      {
        id: "security",
        heading: "Security",
        body: [
          "Passwords and sign-in tokens are stored only as hashes, so even a copy of the database reveals neither. Scripts can't read the sign-in cookie. No system is completely secure, so use a password you don't use anywhere else.",
        ],
      },
      {
        id: "children",
        heading: "Children",
        body: [
          "This site isn't intended for children. If you're under the age at which you can consent to the use of your data where you live (for example 16 in some EU countries, or 14 in Korea and China), don't create an account.",
        ],
      },
      {
        id: "changes",
        heading: "Changes to this policy",
        body: [
          "When this policy changes, the date at the top of this page changes with it.",
        ],
      },
    ],
  },
  terms: {
    title: "Terms of Use",
    summary:
      "These terms apply when you use this BoothDex site, run by {operator}. By using the site, you agree to them.",
    sections: [
      {
        id: "about",
        heading: "About BoothDex",
        body: [
          "BoothDex is an independent search index of items sold on BOOTH. It isn't affiliated with, endorsed by or sponsored by pixiv Inc., BOOTH or VRChat Inc. BOOTH and VRChat are trademarks of their respective owners.",
        ],
      },
      {
        id: "content",
        heading: "Item information",
        body: [
          "Item names, descriptions, images and prices belong to their creators and shops. They're shown here to help you find items, and every item links to its page on BOOTH.",
          "Information here may be out of date or wrong, and categories, avatar compatibility and translations are partly generated by AI. Check the BOOTH page before you buy.",
          "Purchases happen on BOOTH, under BOOTH's terms. This site isn't part of them and can't help with orders, payments or refunds.",
        ],
      },
      {
        id: "creators",
        heading: "For creators",
        body: [
          "Item information is copied from BOOTH and updated automatically, so changes you make on BOOTH appear here after the next update. Items no longer sold on BOOTH may stay here as archived entries.",
          "If you have a concern about how your items appear on this site, send the item's link to {contact}.",
        ],
      },
      {
        id: "accounts",
        heading: "Your account",
        body: [
          "Keep your password to yourself. You're responsible for what happens under your account.",
          "We may disable accounts that break these terms. You can stop using the site at any time and ask us to delete your account, as described in the {privacy}.",
        ],
      },
      {
        id: "use",
        heading: "Acceptable use",
        body: [
          {
            list: [
              "Don't attack the site or try to get around its rate limits or access controls.",
              "Don't scrape pages at a pace that burdens the site. Use the public API and RSS feeds within their limits instead.",
              "Don't use the site to break the law or violate the rights of others.",
            ],
          },
        ],
      },
      {
        id: "software",
        heading: "Open-source software",
        body: [
          "BoothDex is free software under the GNU Affero General Public License v3.0 or later. You can get the source code at {source}.",
        ],
      },
      {
        id: "warranty",
        heading: "No warranty",
        body: [
          "The site is provided as is, without warranties of any kind. To the extent the law allows, {operator} isn't liable for losses from using the site or relying on its information. Nothing in these terms limits rights you have under consumer protection laws.",
        ],
      },
      {
        id: "changes",
        heading: "Changes to these terms",
        body: [
          "These terms may change. The date at the top of this page shows the latest version, and if you keep using the site after a change, you accept the new terms.",
        ],
      },
      {
        id: "contact",
        heading: "Contact",
        body: ["For questions about these terms, contact {contact}."],
      },
    ],
  },
};

export default texts;
