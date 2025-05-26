CREATE TABLE "chart" (
	"project_id" TEXT NOT NULL,
	"title" TEXT NOT NULL,
	"tab" TEXT NOT NULL,
	"json" TEXT NOT NULL,
	"date" TEXT NOT NULL,
	PRIMARY KEY("project_id", "title", "tab")
) WITHOUT ROWID;
