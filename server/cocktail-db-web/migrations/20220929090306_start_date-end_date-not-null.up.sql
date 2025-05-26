CREATE TABLE IF NOT EXISTS "new_project" (
  "project_id"	TEXT NOT NULL,
  "title"	TEXT NOT NULL,
  "hashtag_count"	INTEGER NOT NULL,
  "event_count"	INTEGER NOT NULL,
  "tweet_count"	INTEGER NOT NULL,
  "updated_at"	INTEGER NOT NULL,
  "start_date" TEXT NOT NULL,
  "end_date" TEXT NOT NULL,
  "corpus_list" TEXT DEFAULT '[]' NOT NULL,
  "hashtag_list" TEXT DEFAULT '[]' NOT NULL,
  "complete_hashtag_list" TEXT DEFAULT '[]' NOT NULL,
  "hidden_hashtag_list" TEXT DEFAULT '[]' NOT NULL,
  "exclude_hashtag_list" TEXT DEFAULT '[]' NOT NULL,
  PRIMARY KEY("project_id")
) WITHOUT ROWID;

INSERT INTO "new_project"
SELECT 
  "project_id",
  "title",
  "hashtag_count",
  "event_count",
  "tweet_count",
  "updated_at",
  coalesce("start_date", date('2020-01-01')),
  coalesce("end_date", date('2020-07-01')),
  "corpus_list",
  "hashtag_list",
  "complete_hashtag_list",
  "hidden_hashtag_list",
  "exclude_hashtag_list"
FROM "project";

DROP TABLE "project";
ALTER TABLE "new_project" RENAME TO "project";
