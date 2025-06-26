DROP TABLE user;
CREATE TABLE user (
    "id" INTEGER PRIMARY KEY,
    "username" TEXT NOT NULL,
    "provider" TEXT,
    "external_id" TEXT
);
