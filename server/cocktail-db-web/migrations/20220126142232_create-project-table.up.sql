CREATE TABLE "project" (
	"project_id"	TEXT NOT NULL,
	"title"	TEXT NOT NULL,
	"hashtag_count"	INTEGER NOT NULL,
	"event_count"	INTEGER NOT NULL,
	"tweet_count"	INTEGER NOT NULL,
	"updated_at"	INTEGER NOT NULL,
	PRIMARY KEY("project_id")
) WITHOUT ROWID;
