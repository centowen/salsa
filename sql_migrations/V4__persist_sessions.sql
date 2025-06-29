CREATE TABLE pending_oauth2 (
    "id" INTEGER PRIMARY KEY,
    "csrf_token" TEXT NOT NULL,
    "provider" TEXT NOT NULL
);

CREATE TABLE session (
    "id" INTEGER PRIMARY KEY,
    "token" TEXT NOT NULL,
    "user_id" INTEGER,

    CONSTRAINT fk_user_id
        FOREIGN KEY ("user_id")
        REFERENCES user ("id")
        ON DELETE CASCADE
);
