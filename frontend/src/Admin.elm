port module Admin exposing (main)

import Browser
import File exposing (File)
import File.Select as Select
import Html exposing (Html, a, button, div, form, h1, h2, input, label, li, option, p, select, span, table, tbody, td, text, textarea, th, thead, tr, ul)
import Html.Attributes exposing (class, href, id, selected, type_, value)
import Html.Events exposing (onClick, onInput, onSubmit, preventDefaultOn)
import Http
import Json.Decode as Decode
import Json.Encode as Encode


port pushUrl : String -> Cmd msg


port urlChanged : (String -> msg) -> Sub msg


type alias Flags =
    { path : String }


type alias Model =
    { path : String
    , error : String
    , notice : String
    , email : String
    , password : String
    , pending : Bool
    , dashboard : Maybe Dashboard
    , inquiries : List Inquiry
    , rows : List Row
    , editor : Maybe Editor
    , media : List Media
    , token : String
    , pendingBody : Maybe Encode.Value
    , pendingPath : String
    , upload : Maybe File
    }


type alias Dashboard =
    { destinations : Int
    , hotels : Int
    , inquiries : Int
    , posts : Int
    , recent : List Inquiry
    , media : List Media
    }


type alias Inquiry =
    { id : String
    , name : String
    , email : String
    , status : String
    , hotelName : String
    , checkIn : String
    , checkOut : String
    }


type alias Row =
    { id : String
    , label : String
    , slug : String
    , status : String
    }


type alias Editor =
    { kind : String
    , id : String
    , fields : List ( String, String )
    , blocks : List Block
    , destinations : List Choice
    , hotels : List Choice
    }


type alias Block =
    { kind : String
    , text : String
    }


type alias Choice =
    { id : String
    , name : String
    }


type alias Media =
    { id : String
    , alt : String
    , url : String
    }


type Msg
    = Navigate String
    | UrlChanged String
    | Edit String String
    | SubmitLogin
    | GotLogin (Result String ())
    | Logout
    | GotDashboard (Result String Dashboard)
    | GotInquiries (Result String (List Inquiry))
    | SetStatus String String
    | GotRows (Result String (List Row))
    | GotEditor (Result String Editor)
    | EditField String String
    | EditBlock Int String
    | AddBlock String
    | SaveEditor
    | PublishEditor
    | GotSave (Result String String)
    | PickFile
    | GotFile File
    | Uploaded (Result String ())
    | GotCsrf (Result String String)
    | Noop


main : Program Flags Model Msg
main =
    Browser.element
        { init = init
        , view = view
        , update = update
        , subscriptions = \_ -> urlChanged UrlChanged
        }


init : Flags -> ( Model, Cmd Msg )
init flags =
    load
        { path = flags.path
        , error = ""
        , notice = ""
        , email = ""
        , password = ""
        , pending = False
        , dashboard = Nothing
        , inquiries = []
        , rows = []
        , editor = Nothing
        , media = []
        , token = ""
        , pendingBody = Nothing
        , pendingPath = ""
        , upload = Nothing
        }


load : Model -> ( Model, Cmd Msg )
load model =
    let
        parts =
            String.split "/" model.path |> List.filter ((/=) "")
    in
    case parts of
        [ "admin", "login" ] ->
            ( model, Cmd.none )

        [ "admin" ] ->
            ( model, getJson "/api/admin/dashboard" dashboardDecoder GotDashboard )

        [ "admin", "inquiries" ] ->
            ( model, getJson "/api/admin/inquiries" (Decode.list inquiryDecoder) GotInquiries )

        [ "admin", "media" ] ->
            ( model, getJson "/api/admin/dashboard" dashboardDecoder GotDashboard )

        [ "admin", kind ] ->
            if List.member kind kinds then
                ( model, getJson ("/api/admin/" ++ kind) (Decode.list rowDecoder) GotRows )

            else
                ( { model | error = "Section not found." }, Cmd.none )

        [ "admin", kind, id ] ->
            ( model, getJson ("/api/admin/" ++ kind ++ "/" ++ id) editorDecoder GotEditor )

        _ ->
            ( model, Cmd.none )


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        Navigate path ->
            load { model | path = path, error = "", notice = "" }
                |> Tuple.mapSecond (\cmd -> Cmd.batch [ pushUrl path, cmd ])

        UrlChanged path ->
            load { model | path = path, error = "" }

        Edit "email" value ->
            ( { model | email = value }, Cmd.none )

        Edit "password" value ->
            ( { model | password = value }, Cmd.none )

        Edit _ _ ->
            ( model, Cmd.none )

        SubmitLogin ->
            ( { model | pending = True, error = "", pendingPath = "/api/admin/login", pendingBody = Just (Encode.object [ ( "email", Encode.string model.email ), ( "password", Encode.string model.password ) ]) }, fetchCsrf )

        GotLogin (Ok ()) ->
            load { model | path = "/admin", pending = False, error = "" }
                |> Tuple.mapSecond (\cmd -> Cmd.batch [ pushUrl "/admin", cmd ])

        GotLogin (Err reason) ->
            ( { model | pending = False, error = reason }, Cmd.none )

        Logout ->
            ( { model | pendingPath = "/api/admin/logout", pendingBody = Just (Encode.object []) }, fetchCsrf )

        GotDashboard (Ok dashboard) ->
            ( { model | dashboard = Just dashboard, media = dashboard.media }, Cmd.none )

        GotDashboard (Err reason) ->
            ( { model | error = reason }, Cmd.none )

        GotInquiries (Ok items) ->
            ( { model | inquiries = items }, Cmd.none )

        GotInquiries (Err reason) ->
            ( { model | error = reason }, Cmd.none )

        SetStatus id status ->
            ( { model | pendingPath = "/api/admin/inquiries/" ++ id ++ "/status", pendingBody = Just (Encode.object [ ( "status", Encode.string status ) ]) }, fetchCsrf )

        GotRows (Ok rows) ->
            ( { model | rows = rows }, Cmd.none )

        GotRows (Err reason) ->
            ( { model | error = reason }, Cmd.none )

        GotEditor (Ok editor) ->
            let
                kind =
                    String.split "/" model.path |> List.filter ((/=) "") |> List.drop 1 |> List.head |> Maybe.withDefault ""
            in
            ( { model | editor = Just { editor | kind = kind } }, Cmd.none )

        GotEditor (Err reason) ->
            ( { model | error = reason }, Cmd.none )

        EditField key next ->
            ( { model | editor = Maybe.map (updateField key next) model.editor }, Cmd.none )

        EditBlock index next ->
            ( { model | editor = Maybe.map (updateBlock index next) model.editor }, Cmd.none )

        AddBlock kind ->
            ( { model | editor = Maybe.map (\editor -> { editor | blocks = editor.blocks ++ [ { kind = kind, text = "" } ] }) model.editor }, Cmd.none )

        SaveEditor ->
            case model.editor of
                Just editor ->
                    ( { model | pendingPath = "/api/admin/" ++ editor.kind, pendingBody = Just (editorBody editor) }, fetchCsrf )

                Nothing ->
                    ( model, Cmd.none )

        PublishEditor ->
            case model.editor of
                Just editor ->
                    ( { model | pendingPath = "/api/admin/" ++ editor.kind ++ "/" ++ editor.id ++ "/publish", pendingBody = Just (Encode.object []) }, fetchCsrf )

                Nothing ->
                    ( model, Cmd.none )

        GotSave (Ok _) ->
            ( { model | notice = "Saved.", pending = False }, Cmd.none )

        GotSave (Err reason) ->
            ( { model | error = reason, pending = False }, Cmd.none )

        PickFile ->
            ( model, Select.file [ "image/*" ] GotFile )

        GotFile file ->
            ( { model | pendingPath = "media", pendingBody = Nothing, upload = Just file }, fetchCsrf )

        Uploaded (Ok ()) ->
            ( { model | notice = "Image uploaded." }, getJson "/api/admin/dashboard" dashboardDecoder GotDashboard )

        Uploaded (Err reason) ->
            ( { model | error = reason }, Cmd.none )

        GotCsrf (Ok token) ->
            sendPending { model | token = token }

        GotCsrf (Err reason) ->
            ( { model | error = reason, pending = False }, Cmd.none )

        Noop ->
            ( model, Cmd.none )


sendPending : Model -> ( Model, Cmd Msg )
sendPending model =
    case ( model.pendingPath, model.pendingBody ) of
        ( "/api/admin/login", Just body ) ->
            ( { model | pendingBody = Nothing }, postJson model.token "/api/admin/login" body (Decode.succeed ()) GotLogin )

        ( "media", _ ) ->
            case model.upload of
                Just file ->
                    ( { model | upload = Nothing }, uploadFile model.token file )

                Nothing ->
                    ( model, Cmd.none )

        ( "/api/admin/logout", _ ) ->
            ( model, postJson model.token "/api/admin/logout" (Encode.object []) (Decode.succeed ()) (\_ -> Navigate "/admin/login") )

        ( path, Just body ) ->
            if String.endsWith "/status" path then
                ( { model | pendingBody = Nothing }, postJson model.token path body (Decode.succeed "") (\_ -> Navigate model.path) )

            else if String.contains "/publish" path then
                ( { model | pendingBody = Nothing }, postJson model.token path body (Decode.field "status" Decode.string) GotSave )

            else
                ( { model | pendingBody = Nothing }, postJson model.token path body (Decode.oneOf [ Decode.field "id" Decode.string, Decode.succeed "ok" ]) GotSave )

        _ ->
            ( model, Cmd.none )


view : Model -> Html Msg
view model =
    if model.path == "/admin/login" then
        loginView model

    else
        shell model


loginView : Model -> Html Msg
loginView model =
    div [ class "admin-login" ]
        [ div [ class "card" ]
            [ div [ class "admin-login__header" ]
                [ p [ class "eyebrow" ] [ text "ELSEWHERE ADMIN" ]
                , h1 [ class "admin-login__title" ] [ text "Welcome back." ]
                ]
            , div [ class "card-content" ]
                [ form [ prevent SubmitLogin ]
                    [ field "admin-email" "Email" model.email "email"
                    , field "admin-password" "Password" model.password "password"
                    , button [ class "button is-primary", type_ "submit", Html.Attributes.disabled model.pending ] [ text "Sign in" ]
                    , errorText model.error
                    ]
                ]
            ]
        ]


shell : Model -> Html Msg
shell model =
    let
        section =
            String.split "/" model.path |> List.filter ((/=) "") |> List.drop 1 |> List.head |> Maybe.withDefault "overview"
    in
    div [ class "admin-shell" ]
        [ div [ class "admin-shell__layout" ]
            [ Html.aside [ class "admin-shell__sidebar" ]
                [ div [ class "admin-shell__sidebar-inner" ]
                    [ div [ class "admin-shell__brand" ]
                        [ a [ href "/", Html.Attributes.attribute "data-native" "true" ] [ text "Elsewhere" ]
                        , p [] [ text "Admin studio" ]
                        ]
                    , Html.nav [ class "admin-shell__nav" ]
                        (List.map (navItem model.path) nav)
                    , button [ class "button is-ghost", onClick Logout ] [ text "Sign out" ]
                    ]
                ]
            , div [ class "admin-shell__main" ]
                [ Html.header [ class "admin-shell__header" ]
                    [ p [ class "eyebrow" ] [ text "ELSEWHERE ADMIN" ]
                    , h1 [] [ text (title section model.path) ]
                    ]
                , errorText model.error
                , if model.notice /= "" then
                    p [ class "notification is-success" ] [ text model.notice ]

                  else
                    text ""
                , pageBody section model
                ]
            ]
        ]


pageBody : String -> Model -> Html Msg
pageBody section model =
    case section of
        "overview" ->
            dashboardView model.dashboard

        "inquiries" ->
            inquiryTable model.inquiries

        "media" ->
            mediaView model

        _ ->
            if List.any (\kind -> String.contains ("/admin/" ++ kind ++ "/") model.path) kinds then
                editorView model.editor

            else
                listing section model.rows


dashboardView : Maybe Dashboard -> Html Msg
dashboardView dashboard =
    case dashboard of
        Nothing ->
            p [] [ text "Loading the studio…" ]

        Just data ->
            div []
                [ div [ class "admin-stats" ]
                    [ stat "Destinations" data.destinations
                    , stat "Hotels" data.hotels
                    , stat "Inquiries" data.inquiries
                    , stat "Journal" data.posts
                    ]
                , h2 [] [ text "Recent inquiries" ]
                , inquiryTable data.recent
                ]


inquiryTable : List Inquiry -> Html Msg
inquiryTable items =
    if List.isEmpty items then
        p [] [ text "No inquiries yet." ]

    else
        table [ class "table is-fullwidth" ]
            [ thead []
                [ tr []
                    [ th [] [ text "Guest" ]
                    , th [] [ text "Hotel" ]
                    , th [] [ text "Dates" ]
                    , th [] [ text "Status" ]
                    ]
                ]
            , tbody []
                (List.map
                    (\item ->
                        tr []
                            [ td [] [ text (item.name ++ " · " ++ item.email) ]
                            , td [] [ text item.hotelName ]
                            , td [] [ text (item.checkIn ++ " → " ++ item.checkOut) ]
                            , td []
                                [ select [ onInput (SetStatus item.id) ]
                                    (List.map
                                        (\status -> option [ value status, selected (status == item.status) ] [ text status ])
                                        [ "new", "contacted", "closed" ]
                                    )
                                ]
                            ]
                    )
                    items
                )
            ]


listing : String -> List Row -> Html Msg
listing kind rows =
    div []
        [ a [ class "button is-primary", href ("/admin/" ++ kind ++ "/new"), onNavigate ("/admin/" ++ kind ++ "/new") ] [ text "New" ]
        , table [ class "table is-fullwidth" ]
            [ thead [] [ tr [] [ th [] [ text "Name" ], th [] [ text "Slug" ], th [] [ text "Status" ] ] ]
            , tbody []
                (List.map
                    (\row ->
                        tr []
                            [ td [] [ a [ href ("/admin/" ++ kind ++ "/" ++ row.id), onNavigate ("/admin/" ++ kind ++ "/" ++ row.id) ] [ text row.label ] ]
                            , td [] [ text row.slug ]
                            , td [] [ text row.status ]
                            ]
                    )
                    rows
                )
            ]
        ]


editorView : Maybe Editor -> Html Msg
editorView maybeEditor =
    case maybeEditor of
        Nothing ->
            p [] [ text "Loading editor…" ]

        Just editor ->
            form [ prevent SaveEditor ]
                (List.map (editorField editor) editor.fields
                    ++ (if editor.kind == "destinations" || editor.kind == "posts" then
                            [ h2 [] [ text "Story blocks" ]
                            , div [] (List.indexedMap blockView editor.blocks)
                            , button [ type_ "button", class "button", onClick (AddBlock "paragraph") ] [ text "Add paragraph" ]
                            , button [ type_ "button", class "button", onClick (AddBlock "heading") ] [ text "Add heading" ]
                            ]

                        else
                            []
                       )
                    ++ [ button [ class "button is-primary", type_ "submit" ] [ text "Save" ]
                       , if editor.id /= "new" then
                            button [ class "button", type_ "button", onClick PublishEditor ] [ text "Toggle publish" ]

                         else
                            text ""
                       ]
                )


editorField : Editor -> ( String, String ) -> Html Msg
editorField editor ( key, current ) =
    div [ class "field" ]
        [ label [ class "label" ] [ text key ]
        , if key == "destinationId" then
            choiceSelect key current editor.destinations

          else if key == "hotelId" then
            choiceSelect key current editor.hotels

          else if key == "summary" || key == "description" || key == "excerpt" || key == "terms" || key == "seoDescription" then
            textarea [ class "textarea", value current, onInput (EditField key) ] []

          else
            input [ class "input", value current, onInput (EditField key) ] []
        ]


choiceSelect : String -> String -> List Choice -> Html Msg
choiceSelect key current choices =
    select [ onInput (EditField key) ]
        (option [ value "", selected (current == "") ] [ text "Choose" ]
            :: List.map (\choice -> option [ value choice.id, selected (choice.id == current) ] [ text choice.name ]) choices
        )


blockView : Int -> Block -> Html Msg
blockView index block =
    div [ class "field" ]
        [ label [ class "label" ] [ text block.kind ]
        , textarea [ class "textarea", value block.text, onInput (EditBlock index) ] []
        ]


mediaView : Model -> Html Msg
mediaView model =
    div []
        [ button [ class "button is-primary", type_ "button", onClick PickFile ] [ text "Upload image" ]
        , ul []
            (List.map
                (\item ->
                    li []
                        [ Html.img [ Html.Attributes.src item.url, Html.Attributes.alt item.alt, Html.Attributes.width 120 ] []
                        , span [] [ text item.alt ]
                        ]
                )
                model.media
            )
        ]


stat : String -> Int -> Html msg
stat labelText count =
    div [ class "card" ] [ div [ class "card-content" ] [ p [] [ text labelText ], h2 [] [ text (String.fromInt count) ] ] ]


nav : List ( String, String )
nav =
    [ ( "Overview", "/admin" )
    , ( "Inquiries", "/admin/inquiries" )
    , ( "Hotels", "/admin/hotels" )
    , ( "Rooms", "/admin/rooms" )
    , ( "Offers", "/admin/offers" )
    , ( "Destinations", "/admin/destinations" )
    , ( "Journal", "/admin/posts" )
    , ( "Media", "/admin/media" )
    ]


kinds : List String
kinds =
    [ "destinations", "hotels", "rooms", "offers", "posts" ]


navItem : String -> ( String, String ) -> Html Msg
navItem current ( labelText, path ) =
    a [ href path, class (if current == path then "is-active" else ""), onNavigate path ] [ text labelText ]


title : String -> String -> String
title section path =
    if String.endsWith "/new" path then
        "New " ++ section

    else if List.length (String.split "/" path) > 3 then
        "Edit " ++ section

    else
        case section of
            "overview" ->
                "Overview"

            "posts" ->
                "Journal"

            other ->
                String.toUpper (String.left 1 other) ++ String.dropLeft 1 other


field : String -> String -> String -> String -> Html Msg
field fieldId labelText current kind =
    div [ class "field" ]
        [ label [ class "label", Html.Attributes.for fieldId ] [ text labelText ]
        , input [ class "input", id fieldId, type_ kind, value current, onInput (Edit (if kind == "password" then "password" else "email")) ] []
        ]


errorText : String -> Html msg
errorText message =
    if message == "" then
        text ""

    else
        p [ class "notification is-danger", Html.Attributes.attribute "role" "alert" ] [ text message ]


onNavigate : String -> Html.Attribute Msg
onNavigate path =
    preventDefaultOn "click" (Decode.succeed ( Navigate path, True ))


prevent : Msg -> Html.Attribute Msg
prevent msg =
    preventDefaultOn "submit" (Decode.succeed ( msg, True ))


fetchCsrf : Cmd Msg
fetchCsrf =
    Http.get
        { url = "/api/csrf"
        , expect = expectJson (Decode.field "csrfToken" Decode.string) GotCsrf
        }


getJson : String -> Decode.Decoder a -> (Result String a -> Msg) -> Cmd Msg
getJson url decoder toMsg =
    Http.get { url = url, expect = expectJson decoder toMsg }


postJson : String -> String -> Encode.Value -> Decode.Decoder a -> (Result String a -> Msg) -> Cmd Msg
postJson token url body decoder toMsg =
    Http.request
        { method = "POST"
        , headers = [ Http.header "X-CSRF-Token" token, Http.header "Content-Type" "application/json" ]
        , url = url
        , body = Http.jsonBody body
        , expect = expectJson decoder toMsg
        , timeout = Nothing
        , tracker = Nothing
        }


expectJson : Decode.Decoder a -> (Result String a -> Msg) -> Http.Expect Msg
expectJson decoder toMsg =
    Http.expectStringResponse toMsg <|
        \response ->
            case response of
                Http.GoodStatus_ _ body ->
                    Decode.decodeString decoder body |> Result.mapError Decode.errorToString

                Http.BadStatus_ metadata body ->
                    if metadata.statusCode == 401 && not (String.contains "/login" metadata.url) then
                        Err "Session expired. Sign in again."

                    else
                        Err body

                _ ->
                    Err "Request failed."


uploadFile : String -> File -> Cmd Msg
uploadFile token file =
    Http.request
        { method = "POST"
        , headers = [ Http.header "X-CSRF-Token" token ]
        , url = "/api/admin/media"
        , body = Http.multipartBody [ Http.filePart "file" file ]
        , expect = expectJson (Decode.succeed ()) Uploaded
        , timeout = Nothing
        , tracker = Nothing
        }


dashboardDecoder : Decode.Decoder Dashboard
dashboardDecoder =
    Decode.map6 Dashboard
        (Decode.field "destinationCount" Decode.int)
        (Decode.field "hotelCount" Decode.int)
        (Decode.field "inquiryCount" Decode.int)
        (Decode.field "postCount" Decode.int)
        (Decode.field "recentInquiries" (Decode.list inquiryDecoder))
        (Decode.field "media" (Decode.list mediaDecoder))


mediaDecoder : Decode.Decoder Media
mediaDecoder =
    Decode.map3 Media
        (Decode.field "id" Decode.string)
        (Decode.field "alt" Decode.string)
        (Decode.oneOf [ Decode.at [ "variants", "thumbnail" ] Decode.string, Decode.succeed "" ])


inquiryDecoder : Decode.Decoder Inquiry
inquiryDecoder =
    Decode.map7 Inquiry
        (Decode.field "id" Decode.string)
        (Decode.field "name" Decode.string)
        (Decode.field "email" Decode.string)
        (Decode.field "status" Decode.string)
        (Decode.oneOf [ Decode.field "hotelName" Decode.string, Decode.succeed "" ])
        (Decode.oneOf [ Decode.field "checkIn" Decode.string, Decode.succeed "" ])
        (Decode.oneOf [ Decode.field "checkOut" Decode.string, Decode.succeed "" ])


rowDecoder : Decode.Decoder Row
rowDecoder =
    Decode.map4 Row
        (Decode.field "id" Decode.string)
        (Decode.field "label" Decode.string)
        (Decode.field "slug" Decode.string)
        (Decode.field "status" Decode.string)


editorDecoder : Decode.Decoder Editor
editorDecoder =
    Decode.map6 Editor
        (Decode.succeed "")
        (Decode.at [ "item", "id" ] Decode.string)
        (Decode.field "item" fieldsDecoder)
        (Decode.at [ "item", "content" ] (Decode.oneOf [ Decode.list blockDecoder, Decode.succeed [] ]))
        (Decode.field "destinations" (Decode.list choiceDecoder))
        (Decode.field "hotels" (Decode.list choiceDecoder))


fieldsDecoder : Decode.Decoder (List ( String, String ))
fieldsDecoder =
    Decode.keyValuePairs Decode.value
        |> Decode.map
            (List.filterMap
                (\( key, item ) ->
                    if List.member key [ "id", "content", "createdAt", "updatedAt", "publishedAt" ] then
                        Nothing

                    else
                        Just ( key, valueToString item )
                )
            )


blockDecoder : Decode.Decoder Block
blockDecoder =
    Decode.map2 Block
        (Decode.oneOf [ Decode.field "type" Decode.string, Decode.succeed "paragraph" ])
        (Decode.oneOf [ Decode.field "text" Decode.string, Decode.succeed "" ])


choiceDecoder : Decode.Decoder Choice
choiceDecoder =
    Decode.map2 Choice (Decode.field "id" Decode.string) (Decode.field "name" Decode.string)


valueToString : Decode.Value -> String
valueToString item =
    Decode.decodeValue Decode.string item
        |> Result.withDefault (Encode.encode 0 item)


updateField : String -> String -> Editor -> Editor
updateField key next editor =
    { editor
        | fields =
            List.map
                (\( name, current ) ->
                    if name == key then
                        ( name, next )

                    else
                        ( name, current )
                )
                editor.fields
    }


updateBlock : Int -> String -> Editor -> Editor
updateBlock index next editor =
    { editor
        | blocks =
            List.indexedMap
                (\position block ->
                    if position == index then
                        { block | text = next }

                    else
                        block
                )
                editor.blocks
    }


editorBody : Editor -> Encode.Value
editorBody editor =
    Encode.object
        (( "id", Encode.string editor.id )
            :: List.map (\( key, current ) -> ( key, Encode.string current )) editor.fields
            ++ [ ( "content"
                 , Encode.list
                    (\block -> Encode.object [ ( "type", Encode.string block.kind ), ( "text", Encode.string block.text ) ])
                    editor.blocks
                 )
               ]
        )

