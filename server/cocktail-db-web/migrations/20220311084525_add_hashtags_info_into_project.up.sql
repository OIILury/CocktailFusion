-- Dates au format "YYYY-MM-DD", avec rust, ce sera chrono::NaiveDate
ALTER TABLE "project" ADD COLUMN start_date TEXT;
ALTER TABLE "project" ADD COLUMN end_date TEXT;

-- Tableau JSON de corpus_id, format UUID
ALTER TABLE "project" ADD COLUMN corpus_list TEXT DEFAULT '[]' NOT NULL;

-- Tableau JSON de hashtags, format texte, car un hashtag est unique
ALTER TABLE "project" ADD COLUMN hashtag_list TEXT DEFAULT '[]' NOT NULL;

-- Tableau JSON de **tous** les hashtags :
-- * les hashtags sélectionnés individuellement
-- * les hashtags compris dans les corpus
-- ce champ sera calculé à partir de `corpus_list` et `hashtag_list`
ALTER TABLE "project" ADD COLUMN complete_hashtag_list TEXT DEFAULT '[]' NOT NULL
