CREATE TABLE "corpus" (
  "corpus_id" TEXT NOT NULL,
  "name" TEXT NOT NULL,
  "hashtag_list" TEXT DEFAULT '[]' NOT NULL,
  PRIMARY KEY("corpus_id")
) WITHOUT ROWID;
